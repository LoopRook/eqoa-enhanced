# EQOA Enhanced mod API: eqoa/1

Round 1 design, 2026-11-08. Implementation target for rounds 2–4; this is not a claim that these APIs exist today. The stability promise begins only after the round 5 audit and owner's release approval.

Owner decisions are in [the plan](../plans/EQOA-MOD-API-PLAN.md). The Lua global and module namespace are `eqoa`; Forge remains a compatibility layer over the same native core. Automation, including botting, is allowed. Writes stay in each mod's private area; designated shared bundled resources remain readable as clarified by the lead (§9). Arbitrary network and process access are excluded. New implementation belongs in fork-owned files; shared files carry neutral hooks only, with the limited neutral read-accessor proposal in §10 requiring footprint review.

## 1. Conventions, types and availability

This specification is normative for the resulting API, not the current implementation. Sources refer to branch base `11e036f`; line numbers identify a symbol at that revision, not an ABI. `P` = `crates/eqoa/src/play.rs`, `F` = `crates/eqoa/src/fork_ext/play.rs`, `A` = `crates/eqoa/src/fork_ext/addons/`, `N` = `crates/net/src/`, `V` = `crates/vi/src/`. Other paths below are relative to `crates/eqoa/src/`. `R` is the read-only Forge reference at `C:/Users/hocka/Github Projects/EQOA Native Rust/run/forge-compat-reference/`, modules under `scripts/modules/frontiers_forge/` at `4026d5c`, UiForge at `6f2d125`.

**Choice: immutable snapshots for reads, ordered events for changes, queued native actions for writes.** One native core feeds both API families; Lua never receives Rust references or memory addresses. A getter is for initial state, drawing, or recovery, not detecting changes by repeated comparison.

All new `eqoa` functions use dot calls and snake_case. **Every array and positional index in the new API is 1-based**, including bars, slots, tabs, menu rows and equipment positions. Enumerated IDs, resource hashes, bit numbers, world IDs and protocol values are identifiers, not positional indexes; they are not incremented. Forge and the two existing legacy extensions keep their old bases (§11).

| Type | Exact meaning |
|---|---|
| `int`, `uint`, `number`, `bool`, `string` | Lua number holding an exact signed integer within ±(2^53−1), nonnegative integer within that limit, finite floating number, Lua boolean, UTF-8 string respectively. NaN/infinity and wrong types fail `invalid_argument`; never silently coerce. Overflow fails `out_of_range`. |
| `Id` | Decimal string for signed 64-bit item-definition and ability IDs, canonical spelling without leading zeros. Avoids Lua 5.1 precision loss. Entity IDs and icon hashes remain exact `uint` in 0..4294967295. World ID is a signed `int`. |
| `T?`, `T[]` | Optional value, dense array of T. Named optional keys are absent/nil when unknown; no zero, −1, empty string or empty object substitute. An available empty list is `{}`. |
| `Vec3`, `Vec2`, `Color` | `{x:number,y:number,z:number}`, `{x:number,y:number}`, `{r:number,g:number,b:number,a:number}`. Colors are linear API components 0..1 as consumed by ImGui; packed ImGui colors retain ImGui's u32 format. |
| `EntityRef`, `AbilityRef`, `ItemRef`, `QuestRef`, `WindowRef` | Opaque, read-only host handles. Internally include world generation, record incarnation, native key and list/window session as applicable. No public constructors or pointer fields. Compare by `==`; never persist handles. |
| `Token`, `RequestId`, `Texture`, `Font`, `Sound` | Opaque host handles scoped to this VM. Tokens and requests are not integers. Textures/fonts can be passed directly to ImGui. Destroyed/foreign/expired handles fail `stale_handle`. |
| `Error` | Read-only `{code:string,message:string,capability:string?,retry_after:number?}`; retry_after is seconds. Codes: `not_yet`, `unsupported`, `unknown_symbol`, `unavailable`, `not_in_world`, `not_found`, `invalid_argument`, `out_of_range`, `stale_handle`, `wrong_window`, `busy`, `cooldown`, `out_of_range_target`, `not_usable`, `permission_denied`, `quota`, `rate_limited`, `cancelled`, `io_error`, `failed`. `not_yet` means a scheduled capability has not landed; `unsupported` means excluded or unavailable on this platform, not a future-round promise. |

Unless explicitly stated otherwise, queries return `T,nil` on success and `nil,Error` on failure. An ordinary missing record returns `nil,nil`; an optional unknown field is nil. Mutation/local resource functions return `true,nil` or `nil,Error`. No implicit exceptions for normal game failures. Misuse of ImGui scopes follows its existing exception/manager containment behavior. Every exported module and record uses a guarded lookup: unknown keys produce a visible diagnostic and nil; callable stubs return `nil,Error{code="not_yet"}` for capabilities with that status, or `nil,Error{code="unsupported"}` for excluded/platform-unsupported capabilities. Known optional fields return nil quietly when data has not arrived; attempting a field whose capability is not_yet/unsupported also diagnoses (a field has no second return value). Methods/fields are enumerated below; there are no implicit raw-struct exports.

Coordinates are native world **game units**, x/z horizontal, y up; north is −z, west is −x. Body/camera headings use radians wrapped [0,2π), increasing north→west; degrees only in explicitly named helpers. Velocity is game units/second (`V/walk.rs:574`, integration uses dt). Time/duration values in `eqoa` are **seconds**; `tick` is a monotonic integer count of 30 Hz game steps, `frame` is a presentation-frame count. Never derive game timers by counting rendered frames. Server timer values (`session.timer`, approximately 983.04 units/second in `P:2791`) are converted in the native adapter, not exposed as seconds unchanged. Cooldown reports are server lockout durations, not server remaining-time guarantees.

Availability labels used below:

| Label | Meaning |
|---|---|
| A | Any time an enabled VM runs, including pregame; display resources also need the ImGui context. |
| W | In a connected, loaded world with real player identity, after the entry dump; no extra positive-HP requirement. Dead players still have state. Use `client.get()` to distinguish loading/disconnected. |
| S | Only after the relevant server record/channel arrives; a presence bit is required, default-initialized native zero is not evidence. |
| O | Only while the corresponding native window/session is open. Closed is an ordinary nil result, not an unsupported feature. |

World generation changes on entry/respawn dump, teleport that resets scene/world, logout or disconnect. Entity incarnation changes when a slot is replaced or an ID reappears after removal. Ability identity is `(generation,id,incarnation)`, not display index or spellbook position. Items are `(generation,list-session,inv_index,incarnation)`; index changes alone do not invalidate them. Quest titles have no server ID: all quest refs invalidate on a log revision. A WindowRef expires on closing or replacing that instance. Old value records remain readable historical copies, but resolving/acting on an expired ref fails. New snapshots preserve full native strings as UTF-8, with invalid native sequences replaced, rather than the current 24/32-byte truncation.

## 2. Versioning, capabilities and diagnostics

Native manifest minimum (existing validation: `A/discovery.rs:432,474,512,563`):

```json
{"schemaVersion":1,"runtime":"lua","api":"eqoa/1","id":"example.hearts","entry":"main.lua"}
```

`eqoa.api` is the string `"eqoa/1"`; `eqoa.revision` is a positive integer additive contract revision, initially 1. `require("eqoa")` and the `eqoa` global reference the same protected table; `require("eqoa.player")`, etc. reference its modules. Every declared module is registered from round 2, even when its round has not landed. Before implementation its entries fail visibly with capabilities `not_yet`. Legacy imports without an api field retain the Forge loader and compatibility `eqoa` extensions, and may query the new global. No claim that the manifest validator alone implements the API.

Optional manifest `entryMode` is `"frame"` (default, preserving every existing manifest) or `"once"` (new event-driven authors). Round 2 validates this field and records it in discovery. New examples use `"entryMode":"once"`. Selecting `"api":"eqoa/1"` alone must **not** change existing entry execution behavior; old native manifests already use that value. Both modes expose the same eqoa API. This avoids breaking Forge-style manifests while giving new authors a single registration entry.

After round 5, changing a name, type, units/base, error/absence behavior, ordering, identity/lifetime, or meaning is breaking; so is changing an optional input's default. New optional fields/functions/events/capabilities are additive; mods must ignore unfamiliar event fields and enum values. Existing numeric enums keep their values. `eqoa/2` is selected per manifest/VM, exposes its own `eqoa` table, and coexists with VMs running `eqoa/1`; version 1 keeps working. No automatic migration of a version-1 package into version 2. Forge imports stay supported independently of the selected native version.

### Capability query (round 2, A)

| Function | Return / behavior |
|---|---|
| `eqoa.capabilities.get(path:string)` | `Capability,nil`; unknown paths return a Capability with `status="unknown"`, without diagnostics. Paths are exact dotted symbols, e.g. `eqoa.player.get`, `eqoa.player.Player.mastery`, `eqoa.actions.cast`, `ImGui.BeginTooltip`, `ImDrawList.AddImageQuad`, `frontiers_forge.cave.Alloc`. |
| `eqoa.capabilities.list(prefix:string="")` | `Capability[]` sorted by path, includes modules, functions, record fields, events (`eqoa.events.<name>`), ImGui members and Forge compatibility entries. Empty prefix lists all. |
| `eqoa.capabilities.has(path:string)` | `bool,nil`: true only for `supported`; independent of whether the player has opened a window. |

`Capability = {path:string,status:"supported"|"partial"|"not_yet"|"unsupported"|"unknown",revision:uint,round:uint?,reason:string?,children:string[]}`. Revision 0 for unimplemented/unknown; round is 2/3/4 for scheduled entries, nil for excluded ones. A module is supported only when all its promised nonoptional children are supported, partial if some are. Every field/query/action/event row inherits its section's round unless marked otherwise. Transient absence never changes support. Platform-dependent sound reports unsupported on a platform without a backend. Capabilities are side-effect free, so probing never creates logs or acquires resources. Manifest `capabilities` strings are informational feature requests shown in the manager; they grant no sandbox privileges and do not enable automation separately.

### Consolidated module and function index

This is the **normative capability spelling index**, not a list inferred from what Rust happens to register. Each comma-separated name below is a function at `eqoa.<module>.<name>`; all have the signatures, units, availability and sources in the referenced section. Round 2 installs all descriptors/stubs; the Round column is the first implementation round. No undeclared callable becomes supported merely because a table lookup succeeds.

| Module | Functions (exact suffixes) | Section / round |
|---|---|---|
| `abilities` | list, get, find, in_range | §4 /2 |
| `actions` | target_target, toggle_stats_box; status, cancel, target, cycle_target, assist, interact, attack, cast, use_hotbar, use_item, equip, destroy, swap_items, quick_slot, weapon_set, prepare_weapon_set, assign_ability, unmemorize_ability, unmemorize_all, arrange_ability, loot, loot_all, buy, sell, bank_move, bank_money, chat, command, move, look, autorun, follow, stop, camera, open_window, close_window, select_row, activate_row, dialog_answer, input_text, drop_quest, mastery_share, mastery_browse, mastery_buy, mastery_refund, buttons, menu_command | §8 /3 target_target/toggle_stats_box; /4 others |
| `bank` | get | §6 /3 |
| `binds` | register | §5 /3 |
| `camera` | get, compass | §7 /4 |
| `capabilities` | get, list, has | §2 /2 |
| `chat` | history, get_channel, channel_name | §4 /2 |
| `client` | get, display | §4–5 /2 |
| `combat` | get_counters | §4 /2 |
| `cooldowns` | get | §4 /2 |
| `effects` | list | §4 /2 |
| `entities` | list, get, find, named, distance | §4 /2 |
| `events` | on, off, once, after | §3 /2 |
| `files` | read, write, remove; list | §5,9 /2; list /4 |
| `fonts` | load, default, release; game | §5 /2; game /3 |
| `gems` | get, is_gem, format | §6 /3 |
| `group` | get | §4 /2 |
| `hotbar` | list | §4 /2 |
| `hud` | hide, show, hidden_by; frame, layout, visible | §5 /2; frame/layout/visible /3 |
| `icons` | resource, ui, info; class, disposition | §5 /2; class/disposition /4 |
| `imgui` | vec2, vec4, plus **every individually named required ImGui function in §5** | §5 /2 and explicitly marked /4 additions |
| `input` | get, claim, release, capture_text, rotate_dpad | §4 /2 |
| `inventory` | get | §6 /3 |
| `items` | get, can_use, stat, slot_info, attack_name, stat_name | §6 /3 |
| `log` | write | §5 /2 |
| `loot` | get | §6 /3 |
| `map` | get, images, load_image, set_waypoint, clear_waypoint | §7 /4; last two aliases use §8 request/result rules |
| `mastery` | get, skills, selected | §7 /4 |
| `math` | bearing, degrees, radians, normalize, signed_angle, facing, is_bit_zero | §5 /2 |
| `merchant` | get, price | §6 /3 |
| `options` | get, set, toggle | §5 /3 |
| `player` | get, experience_for_level; stats | §4–5 /2; stats /3; Player.mastery field /4 |
| `quests` | list, get | §7 /4 |
| `settings` | load, save, on_save, on_load | §5 /2 |
| `sound` | load, play, stop, set_volume, is_playing, release | §5 /2 if native backend exists |
| `target` | get | §4 /2 |
| `text` | localized | §5 /2 |
| `textures` | load, rgba, release | §5 /2 PNG/RGBA; load JPEG format capability /4 |
| `windows` | get, find, get_window, format_hint, parse_hint | §7 /4 |
| `WeaponSets` (legacy casing) | IsAvailable, GetCount, GetSelected, GetIcon, GetBadge, GetName, IsEmpty, Use | §11 /2 compatibility retention |

Root value paths: `eqoa.api`, `eqoa.revision`; root legacy function `eqoa.ChatHistory`. Constant paths: `eqoa.abilities.scope.<key>`, `eqoa.entities.status.<key>`, `eqoa.items.slots.<key>`, `eqoa.items.stats.<key>`, `eqoa.items.attack_types.<key>`; keys/values are defined in §4/6. The non-callable format descriptor `eqoa.textures.formats.jpeg` is supported only in round 4; PNG/RGBA descriptors are round 2.

Record-field paths have the exact form **`eqoa.<home>.<Type>.<field>`**, using the following home/type index and each type's complete field schema in §1–8. These paths are descriptor names, not Lua constructors or module members. Named optional fields are included even when nil at runtime; nested records get their own type paths, e.g. `eqoa.player.Attributes.total` then `eqoa.player.SevenStats.strength`. Array entries do not create numeric capability paths. Anonymous result shapes in earlier tables are assigned the type names here without changing their fields.

| Home | Record types (field schemas in the indicated sections) |
|---|---|
| capabilities | Capability (§2) |
| client | Client (§4), DisplayMetrics (§5 display result) |
| events | Event (§3 envelope); named event descriptors are `eqoa.events.<event-name>` for **every exact name in §3's catalog**, never shortened; payloads reference the record type below or the exact inline data schema in that row |
| player | Player, Vital, Attributes, SevenStats, Resists, Location (§4) |
| entities | Entity (§4) |
| group | Group, Member, RaidMember, Invite (§4) |
| abilities | Ability, RangeResult (in_range's in_range/distance fields, §4) |
| hotbar | Hotbar, HotbarSlot (§4) |
| cooldowns | Cooldown (§4) |
| effects | Effect (§4) |
| chat | ChatMessage (§4) |
| combat | CombatHit, Counters (received/dropped, §4) |
| input | InputState (focused/pad/text_capture), Pad, RawAxes (x/y, §4) |
| icons | IconOptions, SurfaceInfo (width/height/source_format, §5) |
| textures | TextureInfo (§5); also returned unchanged by icons/map |
| fonts | FontInfo (§5) |
| sound | PlayOptions (loop/volume, §5) |
| files | FileEntry (§5) |
| inventory | Container, ContainerView, Tab (§6); bank/loot/merchant use these same schema paths, not copies |
| merchant | MerchantState, MerchantOption (§6) |
| items | Item, ItemStat, CanUseResult (allowed/reasons), SlotInfo (id/name/order, §6) |
| gems | Gem (§6) |
| quests | Quest (§7) |
| mastery | Mastery, MasterySkill, MasteryNode (§7) |
| camera | Camera (§7) |
| windows | Windows, Window, MenuRow, ButtonHint, Dialog (§7; tagged union fields are the union of documented variants) |
| map | MapState, Bounds, MapImage, Waypoint (§7) |
| actions | ActionResult (§8) |

Shared primitive record schemas: `eqoa.Vec2.x/y`, `eqoa.Vec3.x/y/z`, `eqoa.Color.r/g/b/a`, `eqoa.Error.code/message/capability/retry_after` (§1); `RGB8` is a fixed 3-element byte array, not a map of extra field capabilities. Event payload field descriptors are `eqoa.events.<event-name>.data.<field>` as enumerated in §3, so e.g. `eqoa.events.cooldown.started.data.lockout` is auditable. Subscription/request/resource handles have no public data fields. ImGui callables have both `eqoa.imgui.<Name>` and `ImGui.<Name>` alias descriptors; DrawList methods use `ImDrawList.<Name>`, each named in §5. Compatibility module/object/constant members use their exact `frontiers_forge.<module>.<member>` paths (§11), plus UiForge/IGraphicsApi globals. Global `bit` retains eight explicitly supported functions `tobit,bnot,band,bor,bxor,lshift,rshift,arshift` (`A/host.rs:122`, `A/bit.rs:54`); capabilities use `bit.<name>`, with other names visibly unsupported. Implementers must compare their exported descriptor set against this index, §3's event catalog and §5/11's per-name lists during round-5 audit, including unknown-symbol and planned-round negative controls.

### Visible diagnostics

Source to extend: `A/manager.rs:344` currently deduplicates text, retains 128 entries and appends to the active mods root's `addons_error.log`; `A/host.rs:219..230` supplies a bounded Lua log queue (64 messages × 4,096 characters). Round 2 must route caught errors, field misses, require failures, subscriber errors and resource failures through one sink, before returning to Lua. A caught `pcall` must not hide unsupported usage.

Each diagnostic records addon ID (host supplied), API version, symbol, phase, error code, first/last time, repeat count, bounded message (2,048 characters), and a concrete reason/fallback. The manager shows an Unsupported/Failed capability badge and expandable message; the log contains the same identity and reason. `nil,Error` with code not_yet or unsupported as above is the defined stub result, including on actions; never queued/success. Missing modules are registered diagnostic tables, not accidental Lua nil crashes.

Deduplication key: `(VM generation, symbol, code)`. Emit the first occurrence immediately to both sinks; aggregate repeats and write at most once per 60 seconds/key, plus a final suppression count on disable/reload. Keep at most 128 visible keys globally; a bounded 256-key per-VM suppression cache prevents per-frame spam. At most 16 new diagnostic keys/second/VM (burst 32); excess go to one visible overflow counter. Rotate the error log at 5 MiB, keep one previous file. Log-write failure stays visible in the manager and client log; do not pretend persistence succeeded. No payload dump of chat, passwords, file contents or credentials in automatic diagnostics.

Ordinary closed windows, unknown health, missing optional assets, server refusal and busy/cooldown action results are defined outcomes. They do not label a working capability unsupported. Invalid arguments and resource/quota failures are diagnosed; optional missing resource probes use one informational manager entry per path. Error/warn author logs are visible; info/debug remain available in expanded logs with normal rate limits.

## 3. Lifecycle and events

**Choice: native changes are captured when applied, then delivered at a safe Lua boundary.** Host snapshot construction is not permission to poll or diff the eight-line chat ring. Current sources are `P:2327` (session pump), `P:4779` (chat commit), `P:5821` (combat), `P:2684` (lockout); current frame dispatch is `A/manager.rs:207`. Read §10 for neutral integration points.

| Function (A, round 2) | Contract |
|---|---|
| `eqoa.events.on(name:string,callback:function)` | `Token,nil`; callback receives one Event. Max 128 subscriptions/VM. Registration order defines same-event subscriber order. Unknown/unsupported names fail visibly. |
| `eqoa.events.off(token:Token)` | `bool,nil`; true removed, false already removed. Wrong-VM token fails stale_handle. Removal inside dispatch suppresses later invocation; a new subscriber starts with the next event. |
| `eqoa.events.once(name:string,callback:function)` | Like on, removed before first invocation, even if callback fails. |
| `eqoa.events.after(seconds:number,callback:function)` | One-shot token, deadline on monotonic game/session-independent host time. Nonnegative seconds, earliest next event boundary; callback receives `timer` event. Cancel via off. Does not busy-wait or create a thread. |

Every Event has `{name:string,seq:uint,time:number,tick:uint,frame:uint,generation:uint,data:Payload}`. Sequence is globally ordered per native session; a per-VM delivery gap is explicit. time is monotonic seconds since client launch; captured at commit, not delivery. Timer events are VM-local sequence entries. No callbacks re-enter Lua from networking, ImGui, audio, filesystem or native mutation functions.

At each client iteration: (1) apply physical input and due actions at the next game-step boundary; (2) apply incoming game events in pump order and run normal simulation; (3) freeze a coherent snapshot and ordered event batch; (4) publish lifecycle transitions and deliver the batch to enabled VMs in addon-ID order, subscribers in registration order; (5) call render callbacks; (6) submit ImGui and perform deferred resource retirement. Actions queued during steps 4/5 run no earlier than the next game step. Resource completions and local UI intents are committed at step 3. State events carry the copy at their own commit; getters during callbacks return the final frozen snapshot, which can be newer. No fabricated ordering among independent packets beyond the client's observed order.

In once mode, execute entry **once per enable cycle** to register callbacks; `eqoa.events.on("render",fn)` draws. In default frame mode execute the entry once per rendered addon frame (`A/manager.rs:269`), with drawing legal in that entry; **do not deliver the render event in frame mode**. Registering render in frame mode returns `nil,Error{code="invalid_argument"}` with a diagnostic explaining that the entry is already the render callback; it never creates a second drawing pass. Other state events and `settings.render` work in both modes. On ordinary disable run disabling/save, cancel subscriptions/timers/actions/claims/sound, retain the VM's settings data; on re-enable execute entry with subscriptions cleared and globals retained. Authors must initialize retained globals deliberately. Frame-mode code must guard one-time registration and re-register after disable; it may instead use once mode. Registering duplicate state handlers explicitly receives duplicate callbacks. Existing Forge scripts remain in frame mode.

On first enable: execute entry, load settings, emit enabled, then a synthetic `world.entered` + `state.reset` if already in world (with `initial=true`). Do not replay past chat/combat. On zone/logout/disconnect: invalidate handles immediately, cancel unsent requests, release input/movement/HUD claims and stop sound; subscriptions remain until disable so the same mod can resume after loading. Events: old-generation `world.leaving`, cancellation results, `state.reset` with no world state, and later new-generation `world.entered` then `state.reset`. Never deliver an old-generation combat/chat event after new entry. Settings and owned file data persist; game-derived resources invalidate on art revision. Player-visible enabled intent is retained across zoning.

### Event catalog

Changed events fire only on committed changes, or once with `initial=true` in reset; lists are not redelivered on every render. `changed:string[]` is a lexically sorted list of top-level record field names. `before`/`after` can be nil on create/remove. The table defines complete payloads in addition to the common Event envelope.

| Name | Payload (`data`) / exact trigger | Source | Round |
|---|---|---|---|
| `enabled`, `disabling` | `{}`; after settings load / before save and cleanup. Disabling callback cannot submit actions. | manager lifecycle `A/manager.rs:443,474,505` | 2 |
| `render`, `settings.render` | `{dt:number}`; each drawn addon frame **in once mode only** / while its manager Settings panel draws (both modes). dt seconds since previous matching callback, 0 first. ImGui is legal only here and frame-mode render entries. | `A/runtime.rs:647..677`, manager frame/settings invocation | 2 |
| `timer` | `{token:Token}`; deadline reached, delivered once. | new fork-owned scheduler | 2 |
| `world.leaving` | `{reason:"zone"\|"respawn"\|"logout"\|"disconnect",world_id:int?}`; before old state is revoked, even if connection falls between frames. | `P:2358,2804`, session/quit flags in `F:131` | 2 |
| `world.entered` | `{world_id:int,initial:bool}`; entry dump applied and loading gate clears. | `P:2804`, `F:131` | 2 |
| `state.reset` | `{in_world:bool,initial:bool}`; getters now give a complete initial state, or unavailable. No individual fabricated add/remove storms on zoning. | frozen snapshot boundary | 2 |
| `player.changed` | `{before:Player?,after:Player?,changed:string[]}`; identity, stats, vitals, movement or flags change. Movement coalesced once per 30 Hz tick. | channel 0x42 `P:2706`, walker physics `F:141`, progression `P:2361,2705` | 2 |
| `target.changed` | `{before:EntityRef?,after:EntityRef?}`; local selected ID changes, not target-info arrival. | `P:3591,4181,5000,4168` | 2 |
| `entity.added`, `entity.changed`, `entity.removed` | `{ref:EntityRef,before:Entity?,after:Entity?,changed:string[]}`; native entity-channel application; changed includes target disposition reply. Local interpolation coalesces by tick. removed carries last known copy. | `N/entity.rs:333`, `P:2329,2716` | 2 |
| `group.changed` | `{before:Group?,after:Group?,changed:string[]}`; roster/leadership/raid/invites/member channel changes. | `P:2362,2366,2414`, `group.rs:88,113` | 2 |
| `abilities.changed`, `hotbar.changed` | `{before:Ability[]?,after:Ability[]?,changed:string[]}` / `{before:Hotbar[]?,after:Hotbar[]?,changed:string[]}`; ability add/remove/reindex/slot/usable, quick slots and carousel edits. `changed` for arrays is `{"records"}`. | `P:2594,2602,2636,2658,2665,2673`, `hudmenu.rs:314` | 2 |
| `cooldown.started`, `cooldown.updated`, `cooldown.ended` | `{ability:AbilityRef?,ability_id:Id?,display_key:uint,lockout:number}`; every 0xCD applied, and whole ability replacement that changes lockout. 0→positive starts, positive→positive differing updates, positive→0 ends; duplicate equal values do not fire. Missing ability keeps key, never guesses ID. | `P:2684`, `N/dump.rs:150`, `F:11` | 2 |
| `effects.changed` | `{before:Effect[]?,after:Effect[]?,changed:{"records"}}`; channel 0x43 update applied, preserving transitions within a frame. | `P:2713`, `N/dump.rs:213` | 2 |
| `chat.message` | `ChatMessage`; each committed logical message, including local system lines; wrapping never creates extra messages. | `P:4779`, server handlers `P:2773`, `chat.rs:26` | 2 |
| `chat.history_scroll` | `{active:bool,steps:int}`; native chat-history input, consumed once/event rather than per draw. Positive steps have the existing history extension's direction. | `A/chat_history.rs:19`, `fork_ext/chat_history.rs` | 2 |
| `combat.hit` | `CombatHit`; every damage indication, including observed third-party events, sign classified once. | `P:5821`, `F:889`, `A/combat.rs:28` | 2 |
| `input.key`, `input.pad` | `{key:string,down:bool,repeat:bool,synthetic:bool}` / `{before:Pad?,after:Pad?}`; physical app key edge / live remapped report after macro+touch merge, before synthetic/rotation injection, changed. No generated OS input. | `A/runtime.rs:192`, `viewer.rs:2710..2772`, report capture immediately after :2752 | 2 |
| `inventory.changed`, `bank.changed`, `loot.changed`, `merchant.changed` | `{before:Container?,after:Container?,changed:string[],cause:string}`; server list/money/item-HP changes, open/close/session changes. cause is native event name or `open`/`close`/`reset`. Inventory tabs/selection change also reports `view`. | `P:2415`, `bankplay.rs:79`, `npcw.rs:78`, `itemmenu.rs:255` | 3 |
| `quests.changed` | `{before:Quest[]?,after:Quest[]?,changed:{"records"}}`; dump or add/remove title applied. | `questlog.rs:44`, `P:2488,2542` | 4 |
| `camera.changed` | `{before:Camera?,after:Camera?,changed:string[]}`; final camera state changes, coalesced once per tick. | `V/camera.rs:21`, `P:3912` | 4 |
| `windows.changed` | `{before:Windows,after:Windows,changed:string[]}`; final native window/focus/dialog state at a client iteration differs; transient intermediate modes within that iteration are intentionally coalesced. | `P:5085`, `F:680` | 4 |
| `map.changed` | `{before:MapState?,after:MapState?,changed:string[]}`; world/zone/waypoint or native image revision changes. Image ready is separately `resource.ready`. | `F:174`, `map/mod.rs:1080,1094` | 4 |
| `mastery.changed` | `{before:Mastery?,after:Mastery?,changed:string[]}`; native mastery reply applied, after cm_msgs processing. | `P:5927`, `classmastery.rs:253` | 4 |
| `resource.ready`, `resource.failed` | `{kind:"texture"\|"font"\|"sound"\|"map_image",handle:Texture\|Font\|Sound?,error:Error?}`; one terminal load outcome per uncached load request. Synchronous texture/sound outcomes are queued for next event boundary; font/map requests queue an outcome when their deferred work finishes. Cached ready hits emit none. | **New broker plumbing required** around `A/textures.rs:327` and `A/sound.rs:922`; font `flush_pending_fonts` (`A/manager.rs:305`) must publish real ready/fallback/error outcomes, not merely flush. These events do not exist today. | 2; map_image 4 |
| `action.result` | `ActionResult` (§8); one dispatch result/request; optional later correlated server outcome is another event with a distinct phase. | native action consumer | 4 |
| `events.dropped` | `{first_seq:uint,last_seq:uint,count:uint,streams:string[]}`; before next normal delivery after bounded queue overflow. | new native event broker | 2 |

Broker retains 1,024 events and 2 MiB of bounded payloads per active VM; dispatch at most 256 events/iteration. Overflow discards oldest, reports the exact gap and emits state.reset before state-dependent delivery resumes. Chat/combat cannot be reconstructed: report loss, never invent replay. State arrays in event payloads may be shared immutable native copies; charge bytes against the broker budget, not one full copy per subscriber. Max 4 million Lua instructions total per VM/client iteration, shared among its callbacks (current per-invocation limit is `A/host.rs:2`; round 2 strengthens total accounting). A failing subscriber is unsubscribed, diagnosed, and other subscribers continue; instruction/heap exhaustion fails and cleans up the whole addon. Disabled VMs receive no events.

## 4. Core game state (round 2)

All state getters read the frozen native core, not Forge tables. Unsupported R4 fields below stay nil with a diagnostic until that capability lands; they are not initialized to zero.

### Client, player, entities and target

| Function | Return, availability and source |
|---|---|
| `eqoa.client.get()` | A: `Client={in_world:bool,phase:"pregame"\|"loading"\|"world"\|"disconnected",generation:uint,tick:uint,frame:uint,time:number,world_id:int?}`. Source `F:131`, `P:5423`, `frames.rs`. |
| `eqoa.player.get()` | W: `Player?`; nil,not_in_world otherwise. Identity `N/session.rs:359`, `N/dump.rs:15`, stats `N/dump.rs:181`, native assembly `F:137`. |
| `eqoa.player.stats()` | W: `Stats?`; nil,not_in_world otherwise. Detailed summary stats computed from base, worn items, masteries and effects. Includes `rows` array and named keys with `current`, `base`, `change`, `shift` ("same"\|"raised"\|"lowered"). |
| `eqoa.entities.list()` | W: `Entity[]`, all populated client-observed records, native slot order; no fixed 24/32 cap. Empty means none observed, not no spawns in the world. `N/entity.rs:298,333`. |
| `eqoa.entities.get(ref:EntityRef)` | W: `Entity?`; stale_handle if expired. |
| `eqoa.entities.find(id:uint)` | W: current `Entity?`, nil,nil if no visible record. Never searches historical map data. |
| `eqoa.entities.named(name:string,exact:bool=true)` | W: `Entity[]`, literal case-insensitive UTF-8 name match (substring when false); no regex/pattern execution. |
| `eqoa.entities.distance(a:Vec3,b:Vec3)` | A: `number,nil`, Euclidean 3D game units. Pure calculation; `A/entity.rs:226` equivalent. |
| `eqoa.target.get()` | W: current `Entity?`; nil,nil for no target or no observed entity. Use player.target_id for a selected ID with no record. Source `P:4181,5000,2329`. |

`Player` fields (all W unless stated):

| Fields / type | Meaning / native source |
|---|---|
| `entity:EntityRef?`, `id:uint`, `name:string`, `class_id:int`, `class_name:string?`, `race_id:int`, `race_name:string?`, `level:uint` | Client self ID, name and protocol class/race IDs, not array positions. `F:171,276,321`, static names `A/player.rs:155`. Unknown class/race keeps ID, nil name. |
| `experience:uint`, `experience_debt:uint` | Raw progression counters `dump.player.exp/p2c`, `N/dump.rs:21`; not percentages. |
| `hp:Vital?`, `power:Vital?` | S, stats channel; `Vital={current:int,maximum:int,base:int}`. Preserve signed values, no max(0) laundering. Current/max HP `F:280` uses health core; power/stat indexes 9/10 `N/dump.rs:181`. Base HP/power are base[8]/base[10] (`F:286,291`). No guessed max if not sent. |
| `attributes:Attributes?` | S: `{base:SevenStats,total:SevenStats}`; SevenStats has `strength,stamina,agility,dexterity,wisdom,intelligence,charisma:int`, stat/base[0..6], `F:294`. |
| `armor:int?`, `resists:Resists?` | S, armor stat[14]; Resists `{base:int,poison:int,disease:int,fire:int,cold:int,lightning:int,arcane:int}`; buffs stat[22..27], base `40 + floor(total.wisdom/7)` as game's display helper. Buff values are bonuses, not total resistance; `F:308`, `A/player.rs:429`. |
| `position:Vec3?`, `velocity:Vec3?`, `speed:number?`, `horizontal_speed:number?`, `heading:number?` | S walker; vector/second, length, xz length, radians. Nil until walker exists; do not use the current zero-velocity fallback as measured motion. `F:141`, `V/walk.rs:47,574`. |
| `target_id:uint?`, `pet_id:uint?`, `auto_attacking:bool`, `dead:bool`, `has_pet:bool` | Native sentinels become nil. `F:156,161,324`; target IDs stay full u32. |
| `fighting:bool?` | S: self creature p263!=0, queried by `Play::self_fighting` (`P:8504..8506`). This is the game's combat flag, used to request battle music (`viewer.rs:5558`, `sound.rs:482`), **not an audible-playback claim**; nil if no self creature. |
| `location:Location?` | S grid/words: `{world_id:int,place_id:int?,compass_id:int?,name:string?,cell_x:int,cell_z:int}`. Cell coordinates are signed grid labels, not Lua array positions. `F:174`, `zones.rs:54`. |
| `mastery:Mastery?` | Round 4, §7; separate capability `eqoa.player.Player.mastery`. |

`Entity={ref:EntityRef,id:uint,name:string,level:uint?,position:Vec3?,hp_fraction:number?,power:number?,kind:"player"|"npc"|"pet"|"other",class_code:uint?,target_id:uint?,status_flags:uint?,poisoned:bool?,diseased:bool?,disposition:uint?,disposition_name:string?}`. Source `N/entity.rs:69,112`, snapshot `F:368`. hp_fraction is p19/255 when known; power is 0..1 fraction from power reader or self stats (nil when not sent). p19=0 is ambiguous dead/out-of-range, so nil unless native metadata establishes a measured value. Do not assert death from zero. `class_code` is creature p256, **not player class**; player is 2, NPC <2, pet 3. Disposition is only known for current target after TargetInfo (0x0010), invalidated immediately on target change; for other entities it is nil. Status bits: `eqoa.entities.status={nameplate_hidden=1,poisoned=4,diseased=8}` (resource icon IDs are in icons, not status flags). kind unknown values preserve class_code and return other. Explicitly do not infer disposition from kind or map history.

### Group

`eqoa.group.get()` (W) returns `Group={in_group:bool,self_leader:bool?,members:Member[],raid:RaidMember[],invites:Invite[]}`. Not grouped is an available object with in_group=false and empty lists, self_leader=nil. `Member={entity:EntityRef?,id:uint,name:string,active:bool,hp_fraction:number?,position:Vec3?,world_id:int?}`; unknown HP remains nil until `hp_known!=0`. Position uses a current actor first, otherwise active channel-40 coordinates in that member's world. Expose world_id even if different; distance/guidance must check it. No fixed eight-member cap in the native API. `RaidMember={id:uint,name:string,hp_fraction:number?,power_fraction:number?}`, `Invite={id:uint,name:string}`. Sources `group.rs:11,29,63,88,113`, `P:2362,2366,2414,7738,8106`.

**Resolved scout issues:** channel-40's s16 coordinates are cast directly to game-unit floats (`group.rs:121`); native map and HUD compare/subtract them directly against actor/camera positions (`P:7738,8112`). No /100 or /256 conversion. Leadership must be derived from the current roster's first member vs self ID; `Group.leader` currently can stay true after a transfer (`group.rs:93`). The adapter must use the roster derivation, not export that stale flag. Raid list uses server-provided bars and carries no invented coordinates.

### Abilities, hotbars, cooldowns, effects

| Function | Return / source |
|---|---|
| `eqoa.abilities.list()` | W: `Ability[]` sorted by numeric native ID, including known spellbook records; `P:2597`, `F:467`. |
| `eqoa.abilities.get(ref:AbilityRef)` | W: `Ability?`, stale_handle on removed/replaced incarnation. |
| `eqoa.abilities.find(id:Id)` | W: `Ability?`, nil,nil if unknown. |
| `eqoa.abilities.in_range(ref:AbilityRef,from:Vec3,to:Vec3)` | W: `{in_range:bool,distance:number}`; 3D distance, self scope or range=0 always in range. `A/ability.rs:273`. This is geometry, not proof of line-of-sight or castability. |
| `eqoa.hotbar.list()` | W: `Hotbar[]` in native carousel order, includes ability, item, weapon and quick-command menus; source `hudmenu.rs:32,48,286,314`, `F:557`. |
| `eqoa.cooldowns.get(ref:AbilityRef)` | W/S: `Cooldown={active:bool,lockout:number,started_at:number?,estimated_remaining:number?}`. Lockout=raw ms/1000, no countdown mutation. Known zero means ready. On entry with active lockout but no edge timestamp, started_at/remaining=nil. Source `P:2684`, `A/ability.rs:54`. |
| `eqoa.effects.list()` | W/S: `Effect[]`; nil,unavailable before first effect state, `{}` after an explicit zero count. Native `N/dump.rs:201,213`, `P:2713`; replace `F:733` default. |

`Ability={ref:AbilityRef,id:Id,name:string,description:string,display_key:int,spellbook_slot:uint?,usable:bool,level:int,range:number,cast_time:number,power_cost:int,icon_background:uint?,icon_foreground:uint?,scope:int,cooldown:number,equipment_mask:uint,lockout:number}`. Source `N/dump.rs:150` fields of the same meaning; `cast_time` and `cooldown` seconds (`R/ability.lua:100,127`); lockout converts milliseconds, is server authoritative. spellbook_slot maps nonnegative a.slot+1, nil if absent. display_key is the **unmodified server lookup key**, not a stable identity or Lua index. scope IDs are the pinned Scope enum (`R/ability.lua:40`); `eqoa.abilities.scope` exports those keys in snake_case with the same numeric values. Unknown scope values retained. usable is a.valid!=0; it is not a server guarantee. Icon sentinel −1 becomes nil.

`Hotbar={index:uint,kind:"abilities"|"items"|"weapon_sets"|"commands"|"other",selected:bool,selected_slot:uint?,slots:HotbarSlot[]}`; `HotbarSlot={index:uint,kind:"empty"|"ability"|"item"|"weapon_set"|"command",ability:AbilityRef?,item:ItemRef?,weapon_set:uint?,command:string?,icon:uint?,ui_icon:uint?}`. ItemRef records become resolvable by §6 in round 3; round 2 can draw real names/icons from existing item snapshots but reports item detail support not_yet. Unknown/native command messages are not exported raw; they use a named safe command registry (§8). Do not clamp selected carousel menu to the first three ability bars (`F:559` does today). Slot art must use actual `V/ui`/hudmenu definition, never a fabricated fifth icon (`A/ability.rs:861`).

Hotbar.index is native carousel menu index+1 (1..number of returned menus); HotbarSlot.index and Hotbar.selected_slot are native slot+1 (1..that bar's slot count). These are the new API positions; Forge alone keeps bar 0..2/slot 0..4.

`Effect={index:uint,icon:uint?,name:string}` for up to eight native effect slots. No duration, caster, stacks or server effect ID exists in `N/dump.rs:201`; do not make them zero fields. `index` is **native slot+1, 1..count (max 8)**, positional, and changes when effects compact. Events carry the full changed list rather than invented stable effect identities. Forge alone converts back to 0..7.

### Chat, combat and input

| Function | Return / availability / source |
|---|---|
| `eqoa.chat.history(limit:uint=100, before_seq:uint?=nil)` | W: `ChatMessage[]`, ascending sequence, latest limit before exclusive before_seq, max 1,000. Logical messages captured at commit; bounded by existing native history size `chat.rs:20,26,68`. A new VM may read retained history but receives only new message events. |
| `eqoa.chat.get_channel()` | W: `"say"\|"group"\|"guild"\|"shout"\|"other"`; `P:4613,5377`. |
| `eqoa.chat.channel_name(channel:string)` | A: string display label: say→Say, group→Party, guild→Guild, shout→Shout, tell→Tell, system→System, other/unknown→Unknown Message Type. Any unrecognized string also returns Unknown Message Type. Pure metadata; pinned Forge name helper is `R/chat.lua:89`, native equivalent `A/chat.rs:142,177`. |
| `eqoa.combat.get_counters()` | A: `{received:uint,dropped:uint}` since this VM enabled; counts observations, not successful attacks. `A/combat.rs:25,28` must gain real loss accounting. |
| `eqoa.input.get()` | A: `{focused:bool,pad:Pad?,text_capture:bool}`; nil pad when no native input source is connected. **Live** report in `App::poll_pad` (`viewer.rs:2710..2772`), read after native macro+touch merges (:2747,:2750), before synthetic input; `input.rs:82`. `A/runtime.rs:192` observes window keys/mouse only. Pregame report can be observed in `viewer.rs:3135..3168` but never receives gameplay actions. `viewer.rs:462..483` is offline playshot code, not a live seam. |
| `eqoa.input.claim(key:string,suppress:bool=false)` | A: Token; physical key names from `winit_key_to_code`/native binds; `suppress=true` only consumes in plain play, never a native dialog. Conflict returns busy; releases still go to original press owner. `A/input.rs:169`, `A/runtime.rs:205`. |
| `eqoa.input.release(token:Token)` | A: bool removed; same idempotence as events.off. |
| `eqoa.input.capture_text(active:bool)` | A: true; per-VM renewable 1-second lease, renewed while custom editor active; never reads other OS apps. Native dialogs take priority. `A/input.rs:10,169`. |
| `eqoa.input.rotate_dpad(active:bool)` | W: true only if the merged report consumer applies the quarter-turn used by Forge; Up→Right→Down→Left→Up. Shared requests OR across VMs; release on world loss/disable. Report unsupported until consumer is implemented. `A/input.rs:52,110` is currently a flag only. |

`ChatMessage={seq:uint,time:number,text:string,color:Color,channel:"say"|"group"|"guild"|"shout"|"tell"|"system"|"unknown",channel_code:int?,sender:string?,outgoing:bool?}`. Channel is based on native opcode/context when known, **not guessed from red color**; unclassified/local lines are system/unknown. Sender/outgoing nil when native parser has no reliable information. Forge red-byte MsgType inference stays in its adapter. Capture unwrapped text at `P:4779` with a commit sequence/time; never reconstruct from the last eight lines or assign session.now at draw time (`A/chat.rs:43` currently does).

`CombatHit={attacker_id:uint,defender_id:uint,amount:uint,kind:"damage"|"heal",color:RGB8?,outgoing:bool,incoming:bool,from_pet:bool,to_pet:bool}`. `RGB8` is a dense **1-based three-byte array `{r,g,b}`**, each integer 0..255: color[1]=red, [2]=green, [3]=blue. It preserves native `Option<[u8;3]>`; it is neither ImGui packed color nor normalized float Color. Color is present **only if `s2.damage_rgb` is enabled and at least three bytes remain** (`P:5827..5830`); nil otherwise, including a short trailer. Black `{0,0,0}` is valid, not absent. Pass the existing parsed rgb through `F:889`, replacing `A/combat.rs:44`'s hardcoded zero; **never read a fourth byte**. Forge packing is specified separately in §11. Native signed amount>0 is heal, negative is damage; expose magnitude, zero is damage 0. outgoing/incoming compare actor IDs with self, from_pet/to_pet compare with actual pet ID; do not claim certainty from patch color. A self-heal is one hit with both direction flags; Forge emits both named callbacks.

`Pad={buttons:string[],left:Vec2,right:Vec2,raw_left:{x:uint,y:uint},raw_right:{x:uint,y:uint}}`. buttons uses lowercase names from `binds.rs:189`, pressed only (new API active-high semantics). New normalized axes are the actual merged **Report.sticks** values in −1..1 (`input.rs:82..88`): right=(sticks[0],sticks[1]), left=(sticks[2],sticks[3]), positive right/down, neutral 0. They are host-normalized input, not game ScaleStick output. Raw axes are the **native libpad bytes** from the same Report::data (`input.rs:239..245`): right=d[4..5], left=d[6..7], each 0..255, produced by native stick_byte (:92, 1.33 scaling/clamp; neutral is **128**, tested at :1646). Do not invent bytes or normalize using a fabricated 127 midpoint. Forge retains its own pinned byte-to-normalized helper in `A/input.rs:272`, not the new API's float values. Snapshot is the live merged report after remapping/macros/touch (`viewer.rs:2727,2747,2750`), before addon rotation/synthetic input; source tags prevent feedback. Virtual leases work without hardware but do not fabricate a connected physical pad in this observation. Raw key names are client-local, not system-wide keylogging. Injection is in §8.

## 5. Presentation and resources (round 2)

### HUD, icons and pure helpers

| Function | Signature / result / availability / source |
|---|---|
| `eqoa.hud.hide(element:string)` | W: Token for this VM's hide claim; **outside W return nil,not_in_world and create no claim**. Native display projection applies while W; world loss revokes token/claim. `A/visibility.rs:31,240`, `fork_ext/hud.rs:21`. |
| `eqoa.hud.show(token:Token)` | A: bool removed; does not release another mod's claim. |
| `eqoa.hud.hidden_by(element:string)` | A: string[] of addon IDs sorted; no arbitrary Lua owner labels. |
| `eqoa.hud.frame(custom_size?:{width:uint,height:uint})` | A: `Frame={width:640,height:448,offset_x:number,offset_y:number,scale_x:number,scale_y:number,side:number,top:number,window_width:uint,window_height:uint}`: 4:3 virtual frame transformation metrics for the current or custom window size. Native source: `fork_ext/addons/hud_layout.rs`. |
| `eqoa.hud.layout(custom_size?:{width:uint,height:uint})` | A: `Layout={player:PlayerLayout,group_members:GroupMemberLayout[],target:TargetLayout,pet:SinglePanelLayout}`: bounding rects for stock HUD elements (`panel`, `health`, `power`, `exp`) transformed into screen coordinates. Native source: `fork_ext/addons/hud_layout.rs`. |
| `eqoa.hud.visible(element?:string)` | W: `bool`: returns whether the specified HUD element (or HUD overall when omitted) is currently visible, respecting `hideui` and mod hide claims. Returns false outside world. Native source: `fork_ext/addons/hud_layout.rs`. |
| `eqoa.hud.text(spec:{x:number,y:number,text:string,color?:number[],scale?:number,align?:"left"|"center"|"right",outline?:bool})` | W: `bool`: queues native HUD text using the game's authentic ArialUI bitmap font in the HUD pass. Bounded per-frame queue; returns nil,not_in_world outside world. Native source: `fork_ext/addons/hud_layer.rs`. |
| `eqoa.hud.gauge(spec:{x:int,y:int,art:int,fill:number,lit?:bool})` | W: `bool`: queues a native gauge (e.g. art 9 mana, 7 health) with authentic frame 136, drawn by native gauge raster. Returns nil,not_in_world outside world. Native source: `fork_ext/addons/hud_layer.rs`. |
| `eqoa.hud.image(spec:{ui_id:int|string,x:int,y:int,w?:int,h?:int,color?:number[]})` | W: `bool`: queues a native UI texture blit (or 3-piece bar when ui_id=25, or panel when ui_id="panel") in the HUD pass. Returns nil,not_in_world outside world. Native source: `fork_ext/addons/hud_layer.rs`. |
| `eqoa.hud.panel(spec:{x:int,y:int,w:int,h:int})` | W: `bool`: queues an authentic game window backing and border frame in the HUD pass. Returns nil,not_in_world outside world. Native source: `fork_ext/addons/hud_layer.rs`. |
| `eqoa.options.get(id:string)` | A: `bool?`: reads the current value of a mod or client toggle setting. Native source: `fork_ext/addons/options.rs`. |
| `eqoa.options.set(id:string,value:bool)` | A: `bool`: updates the value of a mod toggle setting. Native source: `fork_ext/addons/options.rs`. |
| `eqoa.options.toggle(spec:{id:string,label?:string,default?:bool})` | A: `Toggle={id:string,label:string,default:bool,value:bool,item_id:int}`: registers an options toggle. Native source: `fork_ext/addons/options.rs`. |
| `eqoa.binds.register(id:string,default_key:string,description:string)` | A: `bool`: registers a mod-owned keybind on the Controls page under `mod.<addon>.<id>`. Native source: `fork_ext/bindpage.rs`. |
| `eqoa.actions.target_target()` | W: `bool,nil`: assists the current target by targeting its target. Nil,not_in_world outside world. Native source: `fork_ext/addons/actions.rs`. |
| `eqoa.actions.toggle_stats_box()` | W: `bool,nil`: toggles the stats box visibility. Nil,not_in_world outside world. Native source: `fork_ext/addons/actions.rs`. |
| `eqoa.player.stats()` | W: `Stats={strength:StatItem,stamina:StatItem,agility:StatItem,dexterity:StatItem,wisdom:StatItem,intelligence:StatItem,charisma:StatItem,armor:StatItem,max_hp:StatItem,max_power:StatItem,fire:StatItem,cold:StatItem,lightning:StatItem,arcane:StatItem,poison:StatItem,disease:StatItem,rows:StatItem[]}` where `StatItem={name:string,base:int,current:int,change:int}`. Nil,not_in_world outside world. Native source: `fork_ext/addons/player_stats.rs`. |
| `eqoa.icons.resource(hash:uint,options:IconOptions={})` | W/S: TextureInfo; direct live ESF cache, automatic ownership (no Acquire prerequisite). `A/icons.rs:667,1271`, `F:42`. |
| `eqoa.icons.ui(id:uint)` | W/S: TextureInfo from current UI atlas (`V/ui.rs:131`, `A/icons.rs:1308`); missing IDs nil,not_found. |
| `eqoa.icons.info(hash:uint)` | W/S: `{width:uint,height:uint,source_format:string?}`; only data actually decoded by ESF, never invented palette counts. |
| `eqoa.icons.class(class_id:int)` | Round 4 W/S: TextureInfo via the class→UI-art mapping `A/visibility.rs:467`, then icons.ui. Unknown class nil,nil. |
| `eqoa.icons.disposition(value:uint)` | Round 4 W/S: TextureInfo via disposition bucket→UI-art `A/visibility.rs:446`; absent/unknown nil,nil. |
| `eqoa.text.localized(id:int)` | W/S: string? from words and current language, `F:222`, `V/strtab.rs`; nil,nil missing entry, unavailable before words load. |
| `eqoa.math.bearing(from:Vec3,to:Vec3)` | A: number radians [0,2π), atan2(−dx,−dz), same point 0. `A/host.rs:191..200` shows the same geometry **in degrees**; remove its `.to_degrees()` and normalize radians, do not copy its return value unchanged. |
| `eqoa.math.degrees(radians:number)` / `radians(degrees:number)` | A: number, pure conversion, not normalized. |
| `eqoa.math.normalize(angle:number)` / `signed_angle(a:number,b:number)` | A: angle [0,2π) / shortest b−a in [−π,π). |
| `eqoa.math.facing(position:Vec3,heading:number,destination:Vec3,tolerance:number)` | A: bool; shortest bearing difference ≤ tolerance radians, tolerance 0..π; same point true. |
| `eqoa.math.is_bit_zero(value:uint,mask:uint)` | A: bool; inputs 0..0xffffffff, `(value & mask)==0`, no shifts/index adjustment. Safe equivalent of `R/util.lua:377`; native band implementation `A/bit.rs:85..105`. The existing global/require `bit` stays available with its existing BitOp normalization; it is not a new eqoa module. |
| `eqoa.player.experience_for_level(level:uint)` | A: uint threshold or nil,nil outside 1..59; use **native** `hud.rs:124` including level≥51 branch. Forge retains pinned table formula (§11). |

`IconOptions={trim:bool=false,trim_transparent:bool=false,trim_color:Color?}`. Existing crop/trim logic `A/icons.rs:46,123`; cache key includes options and art revision. `TextureInfo={texture:Texture,width:uint,height:uint}` in decoded source pixels. Source_format optional when decoder lacks original metadata. Class and disposition **ID lookup tables** can be present at round 2, but class/disposition TextureInfo capability is not supported until round 4. No second drawing helper API: render these textures with ImGui.Image.

HUD element string enum and **normative expansion** to `A/visibility.rs:14..29` leaves:

| Public element | Native claim leaves |
|---|---|
| `health` | health_bar |
| `power` | power_bar |
| `experience_main` | main_exp_bar |
| `experience_secondary` | secondary_exp_bar |
| `compass` | compass_face, compass_back |
| `target` | target_nameplate |
| `group_panel` | group_member_panel |
| `group_markers` | group_compass_marks |
| `pet` | pet_panel |
| `effects` | active_effects |
| `hotbar` | hotbar_draw_compact, hotbar_draw_expanded |
| `chat` | chat_window |
| `all` | union of the above **14 distinct leaves** |

One Token owns every leaf in its expansion; show releases them atomically. `hidden_by(element)` returns the sorted **deduplicated union** of addon IDs claiming any expanded leaf, including for compass/hotbar/all; it does not assert that every leaf is hidden. Native registry leaf spellings are not public element arguments; reject them with invalid_argument rather than applying a partial mapping. Forge health/power, experience and group compounds expand through the same registry. No element hides native dialogs, map controls or Addon Manager. Clear claims immediately on failure/disable/world loss; show of a revoked token returns false,nil. Installing a mod must not permanently change saved native HUD options.

### Drawing: ImGui is the API

`eqoa.imgui` is the same guarded object as global `ImGui`, not another renderer. `eqoa.imgui.vec2(x:number,y:number)` and `.vec4(x:number,y:number,z:number,w:number)` are exact aliases of `ImVec2`/`ImVec4`; existing globals/enums remain. Positions/sizes/font sizes are **physical output pixels**, independent of game's render scale. `eqoa.client.display()` returns `{width:uint,height:uint,render_scale:uint}` (A). Width/height come from the actual final-target texture extent used for ImGui (`A/runtime.rs:659..663`); ImGui.GetDisplaySize agrees. `render_scale` is the **requested native setting** 1/2/4 from `settings::client().render_scale` (`settings.rs:734..740`, applied by `viewer.rs:3241`), not a UI scale or guaranteed effective offscreen factor (the renderer may fit it down). There is no ui_scale field or fabricated 1.0 value. Mod UI scaling is the author's choice; no multiplying addon coordinates by 2/4 render scale.

ImGui functions retain pinned Lua call names, defaults, overloads, enums and return ordering, not the snake_case convention of new game modules. The normative signature/enum authority is the **actual binding lambdas** in `R/UiForge/externals/sol2_ImGui_Bindings/sol_ImGui.h` and native implemented overloads in `A/bridge.rs`, `bridge_widgets.rs`, `bridge_drawlist.rs`, rather than C++ pointer signatures. Pin these versions in the round-5 author docs; no dependency on an arbitrary future ImGui version. The following is the required bounded subset; other names have `unsupported` capability and visible stubs until added. This is an explicit support boundary, not an assurance that all ~253 reference bindings exist.

Round 2 preserves the native bridge's exported windows/layout/text/widgets/style/query functions (including all their already-supported overloads):

| Area | Required callable names |
|---|---|
| Windows/layout | Begin, End, BeginChild, EndChild, BeginMenuBar, EndMenuBar, BeginMainMenuBar, EndMainMenuBar, BeginMenu, EndMenu, MenuItem, BeginTabBar, EndTabBar, BeginTabItem, EndTabItem, BeginGroup, EndGroup, Indent, Unindent, SameLine, Dummy, SetCursorPos, SetCursorScreenPos, GetCursorPos, GetCursorScreenPos, GetContentRegionAvail, GetWindowPos, SetWindowPos, SetNextWindowPos, SetNextWindowSize, GetScrollY, GetScrollMaxY, SetScrollY, SetScrollHereY, IsWindowFocused, IsWindowHovered, PushID, PopID, PushClipRect, PopClipRect, SetNextItemWidth |
| Text/widgets | Text, TextColored, TextDisabled, TextWrapped, TextUnformatted, BulletText, Separator, SetTooltip, CalcTextSize, Button, InvisibleButton, Selectable, Checkbox, RadioButton, SliderInt, SliderFloat, ColorEdit3, ColorEdit4, InputText, InputTextWithHint, InputInt, Combo, BeginCombo, EndCombo, BeginPopup, EndPopup, OpenPopup, TreeNode, TreePop, CollapsingHeader, ProgressBar, Image, PushTextWrapPos, PopTextWrapPos |
| Style/query | PushStyleColor, PopStyleColor, PushFont, PopFont, SetWindowFontScale, GetColorU32, GetDisplaySize, GetTime, IsItemActive, IsItemHovered, IsMouseDragging, IsMouseHoveringRect, SetKeyboardFocusHere, GetFrameHeight, GetTextLineHeight, GetTextLineHeightWithSpacing, GetBackgroundDrawList, GetWindowDrawList |

Specific round-4 additions (all use actual ImGui IO/input state and existing frame guards):

| Signature | Return / semantics |
|---|---|
| `BeginTooltip()` / `EndTooltip()` | no return; balanced tooltip scope. |
| `BeginItemTooltip()` | bool; EndTooltip only on true, item-hover policy from ImGui. |
| `GetMousePos()` / `GetMousePosOnOpeningCurrentPopup()` | **x:number,y:number tuple**, output pixels (`sol_ImGui.h:1750`), not an ImVec2 object. |
| `GetMouseWheel()` | **vertical:number,horizontal:number tuple**, native ImGui wheel units this render frame (`sol_ImGui.h:83`), not accumulated game ticks. |
| `GetMouseDragDelta(button:int=0,threshold:number=-1)` / `ResetMouseDragDelta(button:int=0)` | x:number,y:number tuple / no return (`sol_ImGui.h:1754`); ImGui mouse **button enum IDs** remain 0 left,1 right,2 middle (not Lua array indexes). |
| `IsMouseDown(button:int)`, `IsMouseClicked(button:int,repeat:bool=false)`, `IsMouseReleased(button:int)`, `IsMouseDoubleClicked(button:int)` | bool, render-frame IO flags. |
| `IsMousePosValid(pos:ImVec2?=nil)` | bool; nil tests current mouse. Deliberately implement real validation rather than pinned wrapper's TODO/always-false stub (`sol_ImGui.h:1748`). |
| `SetMousePos(pos:ImVec2)` / `SetMouseCursor(cursor:int)` | no return; ImGui-local pointer/cursor request only, no OS cursor warp outside the game. |
| `PushStyleVar(id:int,value:number)` or `PushStyleVar(id:int,x:number,y:number)` / `PopStyleVar(count:int=1)` | no return; exact pinned scalar/vector-as-two-numbers overloads (`sol_ImGui.h:155`), checked against enum; balanced scopes. |
| `PushItemWidth(width:number)` / `PopItemWidth()` | no return, output pixels, balanced scopes. |
| `SetNextWindowFocus()` / `SetWindowFocus(name:string?=nil)` | no return, addon-owned ImGui windows only. |

DrawList handles from ImGui are valid only for the current frame epoch and originating window scope. Round 2 methods: PushClipRect, PopClipRect, AddLine, AddRect, AddRectFilled, AddCircle, AddCircleFilled, AddNgon, AddNgonFilled, AddTriangle, AddTriangleFilled, AddText, AddImage, AddImageRounded (`A/bridge_drawlist.rs:220`). Round 4 adds `AddImageQuad(texture:Texture,p1:ImVec2,p2:ImVec2,p3:ImVec2,p4:ImVec2,uv1:ImVec2=(0,0),uv2=(1,0),uv3=(1,1),uv4=(0,1),color:uint=0xffffffff)` → no return, calls real ImDrawList quadrilateral; add `AddQuad(p1:ImVec2,p2:ImVec2,p3:ImVec2,p4:ImVec2,color:uint,thickness:number=1)` and `AddQuadFilled(p1:ImVec2,p2:ImVec2,p3:ImVec2,p4:ImVec2,color:uint)` → no return. Text overloads remain `(pos,color,text)` and `(font,size,pos,color,text)`; round 4 also accepts trailing wrap_width:number=0 on the font form. All positions/widths are pixels; UVs normalized; texture handles must be alive and same VM.

Complete the remaining **safe** pinned DrawList operations in round 4 (`sol_ImGui.h:2541..2595`): `PushClipRectFullScreen()`, `PushTextureID(texture:Texture)`, `PopTextureID()`, `PathClear()`, `PathLineTo(point:ImVec2)`, `PathRect(min:ImVec2,max:ImVec2,rounding:number=0,flags:int=0)`, `PathArcTo(center:ImVec2,radius:number,min_angle:number,max_angle:number,segments:uint=0)`, `PathBezierCubicCurveTo(p2:ImVec2,p3:ImVec2,p4:ImVec2,segments:uint=0)`, `PathStroke(color:uint,flags:int=0,thickness:number=1)`, `PathFillConvex(color:uint)`, and `ImGui.GetForegroundDrawList()->DrawList`. All methods return no values, except the draw-list getter. Angles radians; radius/thickness/rounding pixels; segments 0 means native tessellation. Paths and texture stacks are frame-scoped, reset on failure, and charged against the existing geometry/scope budgets before native allocation. Texture stack accepts owned handles, never raw pointer IDs. Pointer/native-callback/GetIO mutable-pointer APIs remain unsupported. No silent no-op drawing.

### Textures, fonts, sound, settings, logging and private files

These functions are A unless an individual asset is W/S. Paths are relative virtual paths scoped to this addon and its permitted shared-resource fallback (§9); no host absolute path returned by the new API.

| Module/function | Args → return / source |
|---|---|
| `eqoa.textures.load(path:string)` | TextureInfo, PNG in round 2, JPEG in round 4; nil,not_found for absent, nil,unsupported for decoder absent. `A/textures.rs:298,327,583`. |
| `eqoa.textures.rgba(bytes:string,width:uint,height:uint)` | TextureInfo from exactly width×height×4 binary bytes; immutable upload. `A/textures.rs:253`. |
| `eqoa.textures.release(texture:Texture?)` | true; nil and repeated release are idempotent, foreign handle error. Retire after current render; saved handles become stale. |
| `eqoa.fonts.load(path:string,size:number=16)` | `FontInfo={font:Font,status:"pending"\|"ready"\|"fallback",reason:string?}`; TTF/OTF, size>0 ≤256 pixels, queued atlas rebuild; failure gives real default-font handle with fallback status and diagnostic. If no context: nil,unavailable. `A/fonts.rs:175,272,326`. |
| `eqoa.fonts.default()` | FontInfo, ready default; `eqoa.fonts.release(font:Font?)` → true, nil/idempotent, release VM reference after frame. Default is immortal. |
| `eqoa.sound.load(path:string)` | Sound; WAV/MP3 via native backend `A/sound.rs:922`. Decode/backend failure nil,Error. |
| `eqoa.sound.play(sound:Sound,options:{loop:bool=false,volume:number=1}={})` | true only when native backend accepts playback, same handle restarts existing voice, not unbounded voices. Volume 0..1. |
| `eqoa.sound.stop(sound:Sound?)`, `set_volume(sound:Sound,volume:number)`, `is_playing(sound:Sound?)`, `release(sound:Sound?)` | true / true / bool / true; nil stop/release idempotent, nil is_playing false. Unsupported backend still diagnoses unsupported, never false pretending silent playback works. Source `A/sound.rs:933`. |
| `eqoa.settings.load()` | `SettingsValue?`; nil,nil if no saved value. No filename arg. `A/manager.rs:474`, `A/storage.rs:24`. |
| `eqoa.settings.save(value:SettingsValue)` | true only after bounded serialization and atomic replacement succeeds, otherwise nil,Error. Same active addon/profile slot as manager Save/Load. |
| `eqoa.settings.on_save(callback:function)` | Token; callback() → SettingsValue for manager Save/disable. One active callback; replacing retires old token. |
| `eqoa.settings.on_load(callback:function)` | Token; callback(value:SettingsValue?) on initial load/manager Load/profile switch; no implicit call during registration. |
| `eqoa.log.write(level:"debug"\|"info"\|"warn"\|"error",message:string)` | true once queued to bounded local sink; no upload. `A/host.rs:219..230`, `A/manager.rs:344`. |
| `eqoa.files.read(path:string)` / `write(path:string,bytes:string)` | binary string / true, atomic write, max 1 MiB; `A/file_io.rs:196,261,334`, new API facade reuses checked filesystem core. |
| `eqoa.files.remove(path:string)` | bool removed, false if not found; own private file or private tombstone for a visible resource fallback (§9). Never changes shared/package backing files or executable code. |
| `eqoa.files.list(dir:string="",glob:string="*")` | `FileEntry[]` sorted by relative UTF-8 path; immediate children only, `*`/`?` filename glob (no path separators), max 1,024 entries. `FileEntry={path:string,kind:"file"\|"directory",size:uint?}`. Forbidden or excessive enumeration fails, never truncated as complete. |

SettingsValue is nil/bool/finite number/string or acyclic table with either string keys or dense positive integer keys, recursively, max depth 32 / serialized 1 MiB. No handles/functions/metatables; reject mixed sparse tables. The active profile comes from the trusted Addon Manager, not arbitrary paths from Lua. Do not change existing profile identity/migration rules in this work (`A/storage.rs`, `named_profiles.rs`). Save/load callbacks use the same store as explicit functions; no second divergent settings cache. Manager-owned profile persistence is a trusted host service scoped to the addon, not permission for the mod to browse config files. File listing/JPEG arrive in round 4 where map compatibility needs them; read/write and settings are round 2.

## 6. Items and transactions (round 3)

**Choice: definition IDs identify kinds of items; ItemRef identifies a particular stack in a particular list.** Numeric packed-list position is display order only. A buy/sell/loot operation resolves the handle again at dispatch; compaction must never change which item it touches. Native item definition and stack sources are `N/dump.rs:58,88,99`, initial lists `N/dump.rs:493`, mutations `P:2415`, `inventory.rs:60`, `npcw.rs:78`, `bankplay.rs:79`. These sources were checked against the scout; bank contents really do arrive in the world-entry dump, not only at a banker.

| Function | Return / availability |
|---|---|
| `eqoa.inventory.get()` | W: Inventory Container; contents/money from dump, view nil unless native item page open. |
| `eqoa.bank.get()` | W: Bank Container, even closed; native bank money dump.player.p38. View only O banker (mode 11 opened by 0x124D). |
| `eqoa.loot.get()` | O: Loot Container or nil,nil if no corpse window (mode 1); never returns a cached closed corpse. |
| `eqoa.merchant.get()` | O/S: Merchant Container after 0xB7; nil,nil closed. Close discards all public wares/session refs, even if native Shop still retains storage. |
| `eqoa.items.get(ref:ItemRef)` | W plus relevant container availability: Item; removed/transferred/replaced/window-expired handle → stale_handle. |
| `eqoa.items.can_use(ref:ItemRef,class_id:int?=nil,race_id:int?=nil,level:int?=nil)` | `{allowed:bool,reasons:string[]}`. Defaults to real local class/race/level; use native inventory::can_use. Reasons ordered `class`,`race`,`level`; only client-known requirements, not a guarantee that server accepts use. `inventory.rs:54`. |
| `eqoa.items.stat(ref:ItemRef,stat_id:int)` | int; 0 when record has no matching stat, never when ref unavailable. Native `Item.stats`, `N/dump.rs:51,84`; first matching entry, preserve server order. |
| `eqoa.items.slot_info(wear_slot:int)` | A/W-localized: `{id:int,name:string?,order:uint?}`; protocol wear-slot ID, native `WEAR_WORDS` (`infotext.rs:16`) through `wear_slot_word` (:32), pinned paperdoll ordering `R/inventory.lua:20`. Unknown ID retains id, nil label/order. |
| `eqoa.items.attack_name(attack_type:int)` / `stat_name(stat_id:int)` | W/S: string? via `infotext::attack_word` (`infotext.rs:38`, using StartData.attack_words) / STAT_WORDS (:13) plus current words lookup. Unknown nil,nil. |
| `eqoa.gems.get(name:string)` | A: `Gem?`, exact case-sensitive key in pinned 80-entry shipped lookup; unknown nil,nil. |
| `eqoa.gems.is_gem(name:string)` | A: bool = get exists. |
| `eqoa.gems.format(name:string,amount:uint,formatter:function?=nil)` | A: string; default `name .. " x" .. amount`, then append `" ("..type..", "..rarity..", "..stat..")"` for known Gem. formatter(name,amount) supplies the first string; error is diagnosed/failed, not swallowed. Correct the pinned lowercase Gems.get typo. |

`Container={kind:"inventory"|"bank"|"loot"|"merchant",revision:uint,open:bool,items:Item[],capacity:uint?,used:uint,remaining:uint?,full:bool?,money:uint?,view:ContainerView?,merchant:MerchantState?}`. Inventory capacity=40, used=packed-list count, remaining=max(40−used,0), full=used≥40; never infer capacity from the vector's allocated capacity. Loot capacity=40 (display cap) but preserve observed records and diagnose malformed overflow. Bank/merchant capacity nil unless native rules specify one; do not export the Forge hard limit as measured native capacity. `money` is carried/banked tunar; loot/merchant nil. Bank money is known closed. Native sources `inventory.rs:12`, `N/dump.rs:24,25,133`, `merchant.rs:49`, `loot.rs:73`.

`ContainerView={tab:uint?,tabs:Tab[],selected:ItemRef?,selected_row:uint?,rows:ItemRef[]}`; native item tabs are `Tab={index:uint,key:"usable"|"equipment"|"charged"|"misc",label:string?}`, exactly that order from `itemmenu.rs:5,254`; **tab and Tab.index are 1..4**, native tab+1, charged=3. Do not call charged “tradeskill” in new API. Selected row is a 1-based UI row in view.rows, not inventory position. Loot's Leave All row has selected_row=nil and no ItemRef; it does not become item 1. Closed view=nil. Bank/loot/merchant tabs={}, tab=nil. Inventory tab rows resolve each native menu entry ID back to the stack key, `itemmenu.rs:90`.

`MerchantState={entity_id:uint,mode:"menu"|"sell"|"buy",greeting:string,selected_option:"buy"|"sell"|"exit"?,options:MerchantOption[],button_hint:string?,pending:bool,sell_rate:number,buy_rate:number}`; `MerchantOption={key:string,label:string?,selected:bool}`. Source `merchant.rs:49,100,118,193`, mode 10 `npcw.rs:144`, close `npcw.rs:237`. In sell mode view.rows/selected refer to inventory; container.items remains wares. Rates are native price multipliers, not currency; quote prices with `eqoa.merchant.price(ref:ItemRef,direction:"buy"|"sell",quantity:uint=1)` → uint tunar, O, through **native rounding functions** `merchant.rs:80` (`merchant_price`, `sell_price`), not Lua floating rounding. Missing money/stock/insufficient quantity returns defined action error at dispatch.

`Item` fields (all real `N/dump.rs:58..97` unless derived below):

| Fields / type | Native mapping, units and absence |
|---|---|
| `ref:ItemRef`, `list:string`, `index:uint`, `inventory_key:uint` | Owning list, current packed position+1, NetItem.inv_index opaque u32 wire key. Never confuse inventory_key with index. |
| `id:Id`, `name:string`, `description:string`, `icon:uint?` | Item.id/name/desc and inventory::item_icon (`inventory.rs:49`, respects expansion flags/icon2). |
| `quantity:uint`, `charges:int`, `equipped:bool`, `equipped_slot:uint?`, `quick_slot:uint?` | NetItem quantity/charges/equip_slot/quick_slot. −1 becomes nil, valid positional slot becomes native+1; equipped = equip_slot≥0. |
| `wear_slot:int`, `slot_name:string?`, `level_required:int`, `range:number`, `max_stack:uint`, `value:uint` | Definition wear_slot (enum ID, not position), level_req, native game units, max_stack and base tunar value. `value` is not merchant quote. Non-equippable wear slot 0/0xffffffff preserved as enum; equippable false. |
| `current_hp:int`, `maximum_hp:int`, `durability:int`, `damage:int`, `attack_type:int`, `attack_name:string?` | NetItem.cur_hp, Item.max_hp/durability/damage/attack_type. Integer item HP/definition durability, **not percent or seconds**; no derived repair percent when maximum_hp=0. |
| `class_mask:uint`, `race_mask:uint`, `allowed_classes:int[]`, `allowed_races:int[]` | Bits encode native class/race IDs; bit n means ID n. Lists contain IDs, not shifted indexes, ascending. All-one mask expands known classes/races. |
| `equippable:bool`, `no_trade:bool`, `no_rent:bool`, `lore:bool` | wear_slot not 0/0xffffffff, no_trade!=0, **rent==0**, lore!=0; match native inspection. |
| `stats:ItemStat[]` | `{id:int,name:string?,value:int}` entries in received order, no unsigned wrap for penalties; native `Item.stats`. Unknown stat IDs keep value and nil name. |

`eqoa.items.slots`, `.stats`, `.attack_types` are protected snake_case constant maps of the pinned `Item.SLOT/STAT/ATTACK` IDs (`R/item.lua:53,91,117`); preserve numeric values, none/no-slot becomes nil only in positional fields. `Gem={type:string,rarity:string,stat:string}` uses the exact pinned lookup values (`R/gems.lua:5..100`); static shipped metadata, not server-authoritative classification. Unknown/server-custom gem names remain unknown.

Round 3 exposes observations and events; **new game transaction actions land in round 4**, as the plan requires. Do not slip auto-loot/purchase packet writers into round 3 to make the item demo look complete. Existing native item UI remains usable for owner testing.

## 7. Quests, camera, windows, map and mastery (round 4)

### Quests and Class Mastery

| Function | Return, availability and source |
|---|---|
| `eqoa.quests.list()` | W: Quest[] in native order, up to eight titles; `Quest={ref:QuestRef,index:uint,title:string}`. `questlog.rs:17,24,44`, `F:734`, `N/dump.rs:505`. |
| `eqoa.quests.get(ref:QuestRef)` | W: Quest or stale_handle. No objective/progress fields: server sends titles only. Local quest guide text is a separate optional catalog, not an assertion about active server progress. |
| `eqoa.mastery.get()` | W/S: Mastery?; world dump (`N/dump.rs:267`) initializes state, applied replies (`P:5927`, `classmastery.rs:253`) update it. Valid even with CM window closed once initialized; flags track individual counters received. |
| `eqoa.mastery.skills()` | W/S: MasterySkill[] after sub-op 8 list, otherwise nil,unavailable. `classmastery.rs:298`. |
| `eqoa.mastery.selected()` | O/S: MasteryNode? after inspect sub-op 9, nil,nil when no inspected node. `classmastery.rs:93,315`. |

`Mastery={available:uint?,spent:uint?,experience_share:number?,progress_raw:int?,pending:bool,stats:int[],bonuses:int[]}`. Available=x0c, **spent=x10 (not total)**. experience_share=x08/100, 0..1 XP allocation; **not percent earned toward the next CM point**. progress_raw=x14 has no verified normalized meaning; expose explicitly raw integer with no percent label. stats/bonuses copy native [29]/[7] as dense 1-based arrays (no invented enum meanings). Unknown individual values are nil until initialized. `pending` means `Part.pending!=0`; no more than one mastery request outstanding. Source `classmastery.rs:253,266,274,323,365,762,873` and `N/dump.rs:267`.

The scout's first uncertainty is resolved by the window's literal “Points Available/Spent” label (`classmastery.rs:873`), sub-op 10's explicit comment (`:323`), and Select's “share of experience, 0..100” (`:365`). Forge GetCMs/GetCMsSpent/GetCMPct map to available/spent/share×100; do not call x10 “total” or x08 “progress.” The pinned Forge addresses are contiguous x08/x0c/x10 (`R/player.lua:148`).

`MasterySkill={id:int,parent_key:int,rank_key:int,name:string,category:uint,cost:int,known:bool,owned:bool,master_class:bool}` from `classmastery::Skill` (:54): id, parent, x00, name, category, cost, known!=0, on and master_class(). Parent is a **rank key**, not necessarily another skill ID. `MasteryNode={id:int,skill_id:int,parent_key:int,name:string,title:string,category:uint,cost:int?,text:string,known:bool,owned:bool,level_required:uint,points_spent_required:int,minimum_stats:int[]}` from Node (:93): id,x08,x0c,name,title,x10, corresponding Skill.cost if found (nil otherwise),text,x05!=0,x04!=0,x11,x18,mins. Do not label opaque Node.x14 as cost. Text strips no game glyph escapes. Unknown categories preserve ID. Actions apply the actual native purchase/known/master-class gates, not an inferred “enough points” only; no can_buy promise without all native gates. Expose no arbitrary 0x1401 sub-op writer.

### Camera and windows

`eqoa.camera.get()` (W/S) returns `Camera={position:Vec3,heading:number,pitch:number,distance:number,mode:int}`: final eye position `walker.eye.position` (`walker.rs:100`, `P:3912`), camera orbit/facing `cam.cur.orbit` (`V/camera.rs:21`), native orbit_distance and mode. Heading is camera heading, not body (`A/host.rs:183` currently returns body for compass). Camera movement is a game action. First person position can equal player position. `eqoa.camera.compass()` → number radians from final eye direction (atan2(−dx,−dz)); W/S, same convention as §1. Forge camera-facing retains orbit.x, while Util compass maps to final eye direction.

`eqoa.windows.get()` (A; outside W returns Windows with mode="none", native_mode=0, busy=false, empty lists) returns `Windows={mode:string,native_mode:int,busy:bool,start_menu_open:bool,any_window_open:bool,active:WindowRef?,windows:Window[]}`. **The following table defines the contract's exact native-mode ID → name mapping**; its names are new API names, not an existing Rust lookup table. `F:745` is only an is_menu_mode predicate and is not its authority. Source references below are native assignments/owners; never interpret listed names by ordinal position.

| Native ID | eqoa mode name | Native assignment / owner |
|---|---|---|
| 0 | `normal` | `P:5162`, normal SetKey routing |
| 1 | `loot` | `npcw.rs:113` |
| 2 | `input` | `P:6265`, edit box |
| 4 | `gm_goto` | `adminplay.rs:263`, owns :78 |
| 5 | `spellbook` | `P:5745` |
| 6 | `recipe_book` | `P:6089` |
| 7 | `response_dialog` | `P:797`, response key routing :842 |
| 9 | `text_dialog` | `P:866`, PrivChat key routing :806 |
| 10 | `merchant` | `npcw.rs:150` |
| 11 | `bank` | `bankplay.rs:87` |
| 12 | `trainer` | `P:5788` (character summary) |
| 13 | `start_menu` | `P:5470`, also the inventory subpage; inspect Window.kind for inventory |
| 14 | `confirm` | `P:5344`, MBox key routing :812 |
| 15 | `trade` | `npcw.rs:333` |
| 16 | `trade_ask` | `npcw.rs:279` |
| 17 | `buddy_request` | `P:7354` |
| 18 | `buddy_list` | `P:5758` → open_panel(18) |
| 19 | `blacksmith` | `support.rs:40,155` |
| 20 | `guild_invite` | `P:2520` → open_guild(20) |
| 21 | `guild` | `P:6566` → open_guild(21) |
| 22 | `quest_log` | `P:5800` |
| 23 | `mail_compose` | `mail.rs:2,79`, open_mail mode |
| 24 | `mail_inbox` | `mail.rs:3,79` |
| 26 | `ignore_list` | `P:5759` → open_panel(26) |
| 27 | `gm_buff_pc` | `adminplay.rs:190`, owns :78 |
| 28 | `mail_tell` | `mail.rs:4,79` |
| 29 | `mail_reply` | `mail.rs:4,79` |
| 30 | `quick_chat` | `P:6837`, window routing :5096; Forge may call this context_menu |
| 31 | `support_request` | `support.rs:3,40` |
| 32 | `support_inbox` | `support.rs:4,40` |
| 33 | `chat_history` | `P:5304`, key routing :5157 |
| 34 | `options` | `P:7530,7541`, includes Controls subpage |
| 35 | `who` | `P:6512` |
| 36 | `fishing` | `fishplay.rs:88` |
| 37 | `experiment` | `P:6093` |
| 38 | `manual_camera` | `P:3762` |
| 39 | `auction_search` | `bankplay.rs:41` |
| 40 | `mastery` | `P:5774` |
| 41 | `factions` | `P:5782` |
| 42 | `group_search` | `liveplay.rs:172` |
| 43 | `duel_invite` | `duelplay.rs:133`, owns :31 |
| 44 | `duel` | `duelplay.rs:152`, owns :31 |
| 60 | `combat_record` | `recordplay.rs:10` MODE constant, assignment :56 |
| every other int (including 3, 8, 25) | `other` | no implemented owner found at this revision; preserve native_mode unchanged |

`native_mode` keeps the actual number; it is an ID, not a position. Map/search/typing overlays do not invent IDs or replace the underlying mode name; they appear as Window records and affect busy/active. `start_menu_open` means actual pause menu visible; `any_window_open` means native window/map/search/typing present, includes overlays; busy is input ownership blocking plain-play hotkeys, not network pending. `F:680..699` is the existing baseline; do not equate busy with health unknown. Outside W the explicit none state above applies.

`Window={ref:WindowRef,kind:string,title:string?,visible:bool,focused:bool,rows:MenuRow[]?,selected_row:uint?,scroll_top:uint?,hints:ButtonHint[],dialog:Dialog?}`. Native per-window WndList sel/top (`wndlist.rs:53,645`), mode state (`P:5085`), dialogs (`dialog.rs:9,45,241`), edit (`editbox.rs:156`) and current view. Rows/hints only when actually known, nil does not imply an empty window. `MenuRow={index:uint,text:string,enabled:bool,item:ItemRef?,command:string?}`; inspect semantic entries, not memory offsets/vtables. `ButtonHint={button:string,label:string}` uses native glyph names.

MenuRow.index and Window.selected_row are 1..#rows, `WndList.sel+1` only for direct lists; native −1 means nil. Window.scroll_top is native WndList.top+1 (first row=1), nil for a window with no such list. Response dialogs expose **visible options** including More/Back/Close (`dialog.rs:97..119`); selected_row is the position of the option whose native index equals Response.sel, not sel+1 when a later page is showing. `eqoa.actions.select_row`/`activate_row` row and dialog_answer option use exactly these 1-based visible positions; the adapter alone converts to native row/answer identifiers. All action bounds are 1..visible count. Changing a page invalidates the old row view; dispatch revalidates the window/view revision.

`Dialog` is tagged union:

| kind | Fields / availability |
|---|---|
| `text` | `{kind="text",text:string,options:string[]}`; PrivChat (mode 9) / response (mode 7) when native page active. Response options are visible 1-based labels including page/Close actions. Empty options for Done-only PrivChat; dialog_answer option=1 means its single Done action. `dialog.rs:9,45,97`, `P:2342,2348,797,861`. |
| `confirm` | `{kind="confirm",text:string,style:"ok"\|"yes_no"\|"cancel"\|"other",style_id:int,selection:uint?}`; MBox.kind/text/sel; kinds 0=OK,1=Yes/No,2=single Cancel (`dialog.rs:237`, `R/window.lua:594`). Yes/No selection is native sel+1, single-button selection=1. |
| `input` | `{kind="input",text:string?,tag:uint,title:string,chat:bool,password:bool}`; EditBox purpose/title, keyboard typing. Password text always nil. Tag is native purpose ID, never arbitrary memory. `P:6385,6400`, `editbox.rs:156`. |

`eqoa.windows.find(kind:string)` → Window? first visible matching kind, nil,nil none. `eqoa.windows.get_window(ref:WindowRef)` → Window or stale_handle. `eqoa.windows.format_hint(button:string,label:string)` → string in game's `|glyph label` syntax; `parse_hint(text:string)` → ButtonHint[] preserving order and unknown literal text as button="text". Both A pure helpers from native glyph table `binds.rs:189`; no raw window tree. Selection methods are actions (§8). Small neutral read-only accessors are unavoidable for private dialog fields; see §10.

### Map data and waypoints

**Choice: expose calibrated native images and coordinates, not a new terrain rasterizer.** Native maps stay the defaults. No change to map theme, terrain generation, guide search, network submissions or route finding is authorized here. Read map handoff before implementing.

| Function | Return / behavior / source |
|---|---|
| `eqoa.map.get()` | W/S: `MapState={world_id:int,world_name:string?,zone:Location?,bounds:Bounds?,revision:uint,waypoint:Waypoint?}`; world from `P:5423`, words/grid `F:174`, `map/mod.rs:694`, cells. |
| `eqoa.map.images(world_id:int)` | W/S: MapImage[] metadata for **already known** native world cells/overview; nil,unavailable if native map has not initialized that world. Never claims a cache miss is an empty world. `map/cells.rs:37,67,85`, `map/build.rs:207,619`. |
| `eqoa.map.load_image(image_id:string)` | TextureInfo when decoded/uploaded, or nil,unavailable while queued; emits resource.ready/failed. ID from images() only; cannot use it as filesystem path. Uses fork-owned native map store (`map/store.rs:19,36`) and owned textures, not renderer's GPU slot cast to a Lua texture. |
| `eqoa.map.set_waypoint(point:{world_id:int,x:number,z:number,label:string?})` | RequestId; same native personal-waypoint service as map click (`map/mod.rs:520,1094`), §8 results. Any logic can call it; no player-click restriction. Label ≤128 UTF-8 bytes, display metadata owned by mod; underlying native pin remains x/z. |
| `eqoa.map.clear_waypoint()` | RequestId; clears this VM's currently owned personal pin only; busy if user/another mod replaced it. No clearing group guidance as side effect. |

`Bounds={x_min:number,z_min:number,x_max:number,z_max:number}`. `MapImage={id:string,world_id:int,kind:"cell"|"overview",bounds:Bounds,width:uint,height:uint,revision:uint,available:bool}`; bounds are actual world calibration, pixels top-left correspond (x_min,z_min), u increases x, v increases z. IDs are opaque strings scoped to native cache/art revision; no raw directories or paths. `Waypoint={world_id:int,x:number,z:number,label:string?,owner:string?}`; nil owner means player-owned. Map image loading may request existing cached/generated native data using its current build budget; must not start full-world work in Lua callback. Decoded/uploaded image charged to normal per-addon texture quota; requests max 8 pending/VM and bounded native work (existing map builder budget). No promise of all-world images before those worlds have been loaded. Missing handmade art remains missing unless the mod ships its own PNG/JPEG. Changing waypoint invokes native persistence/search invalidation; the fork-owned adapter tracks owner and drops ownership on native user change, arrival, deletion, or disable. On disable clear only a still-owned mod pin; player pins survive. Group chat waypoint creation is an explicit game chat action, not an implied map operation.

## 8. Game actions (round 4)

**Automation is allowed, including full botting.** All functions below can be called from an event, timer or render callback without a player gesture. They use ordinary client checks and the player's existing native command/control path. No packet-builder API, teleport, stat edit, cooldown bypass, inventory fabrication, server permission elevation, or new pathfinding is provided. A mod can implement navigation using observed coordinates and movement inputs.

### Requests, phases, rate and arbitration

Each action returns `RequestId,nil` once validated and **queued**, or `nil,Error` when it could not queue; queued is not a success result. `eqoa.actions.status(id:RequestId)` → `ActionResult?` (A), `eqoa.actions.cancel(id:RequestId)` → bool (only unsent requests cancellable). Retain latest 256 results/VM; status for an evicted own ID is nil,not_found. No polling needed: subscribe to action.result.

`ActionResult={request:RequestId,action:string,phase:"dispatch"|"server",status:"applied"|"sent"|"rejected"|"cancelled"|"confirmed"|"unknown",error:Error?}`. Every queued action gets exactly one dispatch result. applied means a local state/input/UI operation was applied; sent means native path sent its request, **not** server acceptance. If native code exposes a unique matching reply (bank pending state, merchant pending reply, loot/inventory key), optionally emit one server phase confirmed/rejected. Ambiguous parallel replies/timeouts yield unknown; do not correlate by “next packet” or claim that a cast succeeded because a cooldown started. No speculative item/money deduction; only native server updates change records. A server timeout after 10 seconds is unknown and releases only the broker's correlation slot; it never forcibly clears native pending flags or retries a purchase.

All requests carry addon identity, generation, enqueue tick, target/item/window refs and a monotonic queue sequence. Revalidate at dispatch; resolve current native keys/indexes there. Default target for target-dependent actions is captured when queued; reject stale_handle if that target disappears or the selection changed before dispatch. An explicit target option first uses the native target_id path, then dispatches the requested action in the same consumer transaction, costing both actions; target remains selected exactly as if the player changed it. No hidden restore race. Ability cast never changes target when the action's scope is self and no target override was given.

Per VM: 32 queued requests, at most 8 dispatched discrete actions per 30 Hz tick, token bucket 30 discrete actions/second with burst 8; global per-client cap 64/tick. Fair round-robin by addon ID, FIFO within VM. Excess enqueue fails quota; insufficient tokens waits at most 1 second then rate_limited. Do not replay a backlog after zoning/unpausing. Native cooldown/swing/transaction gates may reject earlier than these protective limits. No wall-clock sleeps on frame thread. Continuous controls renew a 0.5-second lease; replacement for the same owner coalesces to the latest value per game tick, not packets every render. At most one transaction outstanding **per native subsystem**, not per addon, for bank, merchant, loot and inventory waiting operations. Different mods get busy; do not pipeline around locks.

Physical movement/look wins a tick and cancels conflicting mod movement/look leases. Claims from two mods conflict with busy, first live lease wins; no last-drawn-wins steering. Disabling, failure, focus loss, zone/logout, disconnect, or a native modal window that locks movement clears held synthetic buttons/axes immediately and emits cancellations. A new lease can resume after focus/window state permits. This player-protection rule does not require an action click or limit automation to macros. Emergency manager disable always works; addons cannot consume its UI inputs.

### Typed action surface

Every row is under `eqoa.actions` except map aliases in §7. All use the request/result contract above. W required unless A is specified. Native permission, dead/rooted/cast, target validity, quantity, class/race/level, pending and window rules remain enforced. An action the native client cannot perform returns unsupported and diagnoses; a native code path that only logs “not ported” is **not** a successful dispatch.

| Function / exact args | Native player path and additional preconditions |
|---|---|
| `target(ref:EntityRef?)` | target_id / clear_target (`P:4181,5000`); nil clears, observed ref must be same generation; selection emits target.changed. |
| `cycle_target(group:bool=false)` | `cycle_target_from` (`P:3606`), actual walker/aspect; preserves native view/range ordering. |
| `assist()` | `assist_target` (`P:3668`), existing target required; server chooses resulting target. |
| `interact()` | `interact_with_target` (`P:879`), native target/distance/type rules; opens loot, dialog, merchant etc only after actual response. |
| `attack(down:bool)` | attack_press / attack_release (`P:4569`, `P:5221`); down is an edge, held according to native autoattack option. Attack on corpse requests its loot as native does. Never bypass attack.may_press. |
| `cast(ability:AbilityRef,options:{target:EntityRef?}={})` | `send_0032` (`P:6785`) same path as hud_key `P:7065`; ability must currently have a usable native slot, positive lockout→cooldown; missing slot→not_usable. Range uses native target_in_range (`P:7184`) when applicable. Do not send a.slot=-1 as u8=255. |
| `use_hotbar(bar:uint,slot:uint)` | resolve exact current menu Entry, route to `hud_key` dispatch `P:7053..7068` by ID/tag; empty→not_found. Revalidate carousel revision, no implicit wrong-bar activation. |
| `use_item(item:ItemRef,options:{target:EntityRef?}={})` | inventory only, `use_item(index,1)` (`P:5485`), class/race/level/use-kind native validation `itemmenu.rs:61`; no quantity argument because native use uses one activation. |
| `equip(item:ItemRef,equipped:bool=true)` | inventory wearable, compare equip state first (already requested state→applied no packet), route same **ItemOut::UseItem(index,0)** as native equip/unequip item-menu command; `itemmenu.rs:761`, `P:5494`. Never assign equip_slot locally. |
| `destroy(item:ItemRef,quantity:uint=1)` | inventory stack count 1..quantity; ItemOut::UseItemOn → `P:5500`; native waiting gate. This direct explicit action is the confirmation; it need not open a player confirmation dialog. |
| `swap_items(a:ItemRef,b:ItemRef)` | both inventory, distinct valid stacks; ItemOut::Swap, `P:5508`, no fabricated destination slot. |
| `quick_slot(item:ItemRef,slot:uint)` | inventory, native quick-slot capacity/type eligibility; ItemOut::QuickSlot `P:5515`, subtract one only at boundary. |
| `weapon_set(index:uint)` | `use_gear_set` via existing fork adapter `F:111`; valid live gear set, mode 0/38. New API base 1, includes bare hands as last native entry; existing WeaponSets.Use stays base 0. |
| `prepare_weapon_set(index:uint,name:string)` | ItemOut::GearSet `P:5521`, set 1..gear::SETS, native name length and eligibility; no action if server lacks gear-set feature. |
| `assign_ability(ability:AbilityRef,slot:uint)` | spellbook `Out::Send0033(display_key,slot−1)` (`spellbook.rs:83,518`), native token/request consumer; send same request the book sends, **not** local ability_slot (`P:6764`, a server-response mutator). |
| `unmemorize_ability(ability:AbilityRef)` / `unmemorize_all()` | native spellbook Send0033 with ability's existing slot (toggle) / Send00FE (`spellbook.rs:446,454`); valid memorized record and native pending gate. |
| `arrange_ability(ability:AbilityRef,before:AbilityRef)` | spellbook Out::Send003A(current display keys), `spellbook.rs:502`; do not mutate local keys or retain them as IDs. |
| `loot(item:ItemRef,quantity:uint=1)` / `loot_all()` | O corpse; LootOut::One(inv_index,count)/All, `npcw.rs:426`, pending/capacity rules from native Loot::key (`loot.rs:117`); no Leave All row confusion. |
| `buy(item:ItemRef,quantity:uint=1)` | O current merchant wares, buy mode; MerchantOut::Buy with **current native ware position** and count (`npcw.rs:443`), money/stock/native pending validation. |
| `sell(item:ItemRef,quantity:uint=1)` | O merchant sell mode, inventory stack; MerchantOut::Sell (`npcw.rs:450`), no_trade/accepted-kind/native restrictions; use native quote, not Item.value. |
| `bank_move(item:ItemRef,destination:"bank"\|"inventory",quantity:uint=1)` | O banker; item in opposite list; BankOut::Move (`bankplay.rs:214`), target banker session and state==0. |
| `bank_money(direction:"deposit"\|"withdraw",amount:uint)` | O banker, positive tunar within source balance; BankOut::MoveMoney `bankplay.rs:224`, state==0, no rounding. |
| `chat(text:string,channel:"current"\|"say"\|"group"\|"guild"\|"shout"="current")` | same `chat_line` (`P:4602`, current adapter `F:813`) **after the synthetic report guard below**, prefix selected channel through existing parser. Text including prefix <128 UTF-16 units, no embedded NUL; fail out_of_range before native silent return. Group/guild membership required as for player. Slash text accepted except report/host-service routes. |
| `command(text:string)` | same guarded chat_line parser; require leading `/`, native known commands/server fallback and GM checks remain. No shell execution. Reject report routes **even when their transport is the authenticated game session**, as well as launch/export/external-service routes (§9). |
| `move(forward:number,strafe:number,options:{walk:bool=false}={})` | normalized −1..1 axes, via **controls::update_controls** (`controls.rs:188,382`) / Walker normal frame, native collision/root/cast/swim/speed; renew 0.5 s lease. No direct position edits. |
| `look(horizontal:number,vertical:number)` | normalized −1..1 right-stick lease via `controls.rs:402`; native camera mode and invert option, no direct transform writes. |
| `autorun(enabled:bool)` | idempotent conditional toggle_autorun (`P:4965`), new desired state only. |
| `follow(member:EntityRef?)` | nil stops; otherwise observed different current group member, target_id then native toggle_follow (`P:4975`) if needed; preserves native follow steering and distance (6 walk /3 stop in `controls.rs:19`). No follow of arbitrary NPC bypass. |
| `stop()` | applied locally: release this VM's leases/held buttons and, if this VM started them, end autorun/follow using native controls_changed (`P:4988`). Does not force velocity to zero or clear physical input/another mod's leases. |
| `camera(options:{heading:number?,pitch:number?,distance:number?,mode:int?})` | at least one field; finite radians/game units/native mode; WalkCamera setters `V/camera.rs:243,256,397` and preset path `camstyle.rs:270`, same clamping/native collisions. Busy if native camera locked. |
| `open_window(kind:string)` | ordinary start-menu/window activation `P:6476`/native start menu messages. Kinds `start_menu,inventory,spellbook,quests,mastery,map,options,group,buddy_list,ignore_list,guild,who,mail_inbox,mail_compose,factions,auction_search,combat_record,recipes,experiment,controls`; server/role features may reject unsupported/unavailable. Bank/loot/merchant require interact(), not force-open. |
| `close_window(ref:WindowRef)` | native cancel/back path `P:5085,5220`, `bankplay.rs:208`, `npcw.rs:429,441`; close only exact current instance, respect pending native locks. |
| `select_row(ref:WindowRef,row:uint)` | current active menu; route native list selection helper + its ordinary change notification; does not activate row. Read actual rows and index bounds; `wndlist.rs:53`, per-window key/pointer paths `P:4436,5085`. |
| `activate_row(ref:WindowRef,row:uint)` | select then native confirm key/action for that menu; enabled row only; executes normal response/quantity/confirmation flow. |
| `dialog_answer(ref:WindowRef,option:uint)` | exact current text-response or confirm; options index 1..N (includes visible Done/Close), route normal window answer/BoxAnswer and server dialog ID, `dialog.rs:45,241`, `P:5085`; never arbitrary server dialog packets. |
| `input_text(ref:WindowRef,text:string,submit:bool=false)` | current native game edit widget only, native typable/length/multiline rules (`editbox.rs:156`, `P:6385,6400`), password fields rejected. submit invokes same edit_done as player. No clipboard/OS injection. |
| `drop_quest(ref:QuestRef)` | current quest revision; `Quests::send_remove(index−1)` (`questlog.rs:65`, `P:6162`) using current stamp. Direct explicit drop needs no manual dialog click. |
| `mastery_share(fraction:number)` | 0..1 in exact .01 steps, Part::select(percent) `classmastery.rs:365`, pending==0 and initialized. |
| `mastery_browse(skill_id:int)` / `mastery_buy(skill_id:int)` / `mastery_refund(skill_id:int)` | Part::browse/send02/send03 (`classmastery.rs:371,376,390`) and **same window gates** (`classmastery.rs:803..850`), pending lock, prerequisites/master-class rule. Require inspected matching node for buy/refund; not blind IDs. |

For native methods returning void today, the action adapter must validate the known early-return conditions and report applied/sent **only when that branch actually executes**. The generic seam can carry a neutral typed CommandOutcome or return dispatch evidence; never infer success from “Rust function returned.” New wrappers/execution stay in the private child adapter, not copied packet logic. Some shared output loops need the neutral factoring in §10.

**Synthetic report guard:** before queue acceptance and again at dispatch, reject `#bug` and `/bug` command tokens case-insensitively (token ends at whitespace or end of line), including after supported say/group/guild/shout/tell/reply wrappers resolve their payload. Return nil,permission_denied, never call `send_bug_note` (`P:4608..4612,4694..4697`), `open_bug_report_typed` (`P:4682..4685`), a report form submit or report-confirmation handler (`P:830..833`). Apply to both eqoa chat/command and Forge SendChatText, and to buttons/menu_command/input_text/dialog_answer paths reaching those semantic operations. Reject when the relevant native support/report editor or confirmation owns the input; ordinary player-typed reporting remains available. Transport inside the game session does **not** exempt reports from §9's exclusion. Do not silently send a different ordinary chat line or report sent/applied after blocking.

### Complete ordinary-input fallback

Not every player menu needs a bespoke mod API. The following lets mods reach **every already implemented game action** (trade offer/accept/cancel, auction bids/sales, crafting/recipes, mail, guild administration, buddy/ignore lists, duel/wager/surrender, trainer, pet orders, emotes, fishing, invite responses, logout), using the same menu rows and buttons as a player. Unsupported native menus remain unsupported; no new game systems are implied.

| Function | Contract and native boundary |
|---|---|
| `eqoa.actions.buttons(buttons:string[],hold_ticks:uint=1)` | W: chord of names in `binds.rs:189` (select,l3,r3,start,up,right,down,left,l2,r2,l1,r1,triangle,circle,cross,square), 1..15 game ticks. Inject at **live App::poll_pad**, after macro OR (:2747) and touch merge (:2750), immediately before console.poll (:2754), then normal native PadKeys→Play::set_key (:2768, `P:5220`). Each synthetic press/release retains Source::Synthetic through the console/dispatch sidecar; capture_button/rebind branches (:2726..2734), pad_rows and any physical-only capture must never see or swallow it. If controls capture or host-only updater/report UI blocks the request, return a real busy/permission_denied dispatch result instead. Source tags also prevent a synthetic key operating the update offer in offer_pad (:2578). No OS events; physical and synthetic holds/releases are accounted separately. A valid virtual lease sets effective input connectivity for console.poll even without hardware; a cancelled final virtual lease must release keys without a fabricated physical disconnect. The :462..483 playshot path is offline verification only, not the gameplay consumer. |
| `eqoa.actions.menu_command(ref:WindowRef,command:string,arg:int=0)` | O: invoke a **currently exposed enabled semantic menu command** from that Window.rows entry, not arbitrary opcode/DoAction u32. String is opaque stable per native command, e.g. `pet.attack`; same native menu handler/`do_action` (`P:6476`) with correct arg. Unknown or not-ported command → unsupported. A ref binds it to the menu/permissions it came from. For trade/quantity fields use input_text and ordinary native buttons. |

The menu adapter enumerates every live MenuEntry (`client_image::MenuEntry` and per-window outputs), translates its message to the command registry, and preserves its native enabled/confirmation flow. There is no public numeric command-ID dispatch and no raw `send`. Named input controls need no fabricated generic window tree. Player input path modules include `npcw.rs:462` trade outputs, `bankplay.rs:248` auction, `P:6478` admin permissions, `P:6482` duel, `P:6486` combat record, `P:6550` pet commands and `P:6677` group actions. These are source-reviewed routing targets, not newly implemented support claims. Round-4 implementers must enumerate routable entries and capabilities, with unsupported entries visibly refused. Automation does not grant GM status or feature words the server did not send.

## 9. Sandbox and execution limits

**Choice (lead decision): private writes plus read-only shared bundled content, enforced in Rust at every boundary.** An addon can read its own code/resources and the designated **bundled `mods/resources` tree** (fonts, icons, support files, including legacy config/notes reads for migration). It can write only its own private data/profile/overlay. It cannot read another mod's folder/private data, arbitrary shared config, server patch, disc path or host directory. The shared bundled tree is an explicit read-only exception to “own folder,” not authority to read the whole mods root. The new API accepts relative paths only. Host-managed settings are a constrained value service, not general filesystem access. Current discovery/media roots (`A/discovery.rs:695..709`) and shared installation (`fork_ext/bundled.rs:253..291`) are retained through the mounts below; current file resolver (`A/file_io.rs:196..234`) must enforce the same per-addon scope.

`UiForge.resources_path` is **explicitly a per-addon virtual overlay mount for the shared resources tree**, not a writable physical shared directory. Retain its legacy host-issued path spelling so unchanged `resources_path .. "\\file.cfg"` works, but resolve it by VM identity: reads check that addon's resource overlay, then the shared bundled tree; writes always land in that addon's resource overlay. All consumers (io, media, list, remove/rename) use this same resolver, including absolute legacy paths with the exact issued prefix. Relative `eqoa.files` paths beginning `resources/` address that mount. `UiForge.scripts_path .. "\\<this-addon>\\resources\\..."` is a **second alias for its package-resource mount**, read overlay first then that package's resource directory, write only its package-resource overlay; another addon name in that path is denied. Relative `package_resources/` addresses that mount; other eqoa.files relative paths address only private data. Bare media resource names retain current package-resource-first, shared-resource-second search, including both overlays. Loader reads of approved shared support modules stay read-only and explicitly allowlisted, never expose arbitrary sibling code/data.

**Migration decision: delete is an addon-private tombstone (“already migrated”).** `os.remove`/files.remove of a currently visible shared fallback config/notes file atomically creates a tombstone in this addon's overlay and returns success; it never deletes/renames the shared backing file. Thereafter read/list in this addon treat that path as absent. Other addons still see their own overlays/fallbacks. If a private overlay copy exists, remove it and retain a tombstone so the shared old copy cannot reappear. A later explicit private write clears that path's tombstone and is read first. For Modern UI notes migration, the new package-resource notes are written/closed successfully before the legacy shared-resource path is tombstoned; avoids_persist's read-then-remove settings path is treated as already migrated by that tombstone, with the migrated state saved through its normal profile callback. A missing already-tombstoned path returns the ordinary not-found result (no unsupported diagnostic). Rename of a readable fallback is private copy-to-destination plus source tombstone, never a shared filesystem rename. Tombstones survive reload/restart and count toward the private storage quota; failure to write them is io_error, not migration success.

| Boundary | Required enforcement |
|---|---|
| Lua VM/loader | `Lua::new_with` whitelist as `A/host.rs:84..87`; no ffi/debug/native loaders/socket/http/process APIs, no `package.loadlib`, binary chunks, arbitrary loadfile/dofile or mutable host registry. Controlled require resolves host modules or text source within own/approved read-only support mount. Instruction budget cannot be defeated with protected pcall (`A/host.rs:280..292`). Do not add async native escape functions. |
| File paths | Central rooted resolver for **files, io, os.remove/rename, require, textures, fonts, sound and listings**. Reject absolute/drive/UNC/device paths, `..`, NUL/control characters, ADS colon and reserved Windows device names, dangling links and reparse/junction components. Resolve parents, open relative to verified root handle without following links, verify final opened handle stays inside root; reject if platform cannot enforce. A check-then-open canonicalize alone is not sufficient against replacement races. Existing no_links is `A/file_io.rs:274`, loaders `A/package.rs:215` and media resolvers must converge on this helper. |
| Private storage | All writes, including legacy resources_path `.cfg`/notes writes and remove/rename metadata, go only to addon data/overlays; cannot edit executable Lua, sibling package or the shared backing tree. 1 MiB/file, 16 buffered io handles, 16 MiB aggregate data/addon including settings/tombstones, listings capped 1,024, atomic replacement. Manager owns data root; Lua cannot supply it. Keep legacy io.open text r/w and os.remove/rename with the overlay semantics above. |
| Imported Forge paths | `UiForge.resources_path` **always** routes to the per-addon shared-resource overlay defined above; reading the designated shared backing is allowed, writing it is forbidden. Own scripts_path/package/resources is the package overlay alias. Provision all mandatory bundled fonts/icons/support assets (no optional “can copy” dependency). modules_path is an own/approved read-only module view; profiles_path is the addon's private profile view. Exact host-issued absolute prefixes are compatibility aliases only; no raw absolute I/O or sibling enumeration. |
| Network | No sockets/HTTP/DNS tools exposed. Actions may use only the **existing authenticated game session and ordinary allowed game commands**; this is game control, not arbitrary networking. No arbitrary endpoint, packet payload, community submission/update/report-upload API. Reports are excluded **even over that session**; §8's synthetic guard blocks #bug, /bug and native report-form/confirmation submission for both API families. |
| Processes/host integration | No os.execute/popen, native DLL loading, URL/shell open, clipboard reads, external file picker, companion launch or process invocation through Lua. Native actions which can trigger those host services are filtered out of menu_command/buttons routes. F3 report export/art disc picker/update installation/browser-open routes are not game actions and return permission_denied if triggered synthetically. The player's own input retains them. |
| Lifetimes | Every resource/action/capture stores VM identity and generation. Disable/failure/world loss cancels held input before next simulation and retires resources safely after render. No callback can keep suppressing manager disable. |

Limits retained/clarified from current code: 16 instantiated VMs (`A/manager.rs:29`), 8 MiB Lua heap (`A/host.rs:88`), 4 million instructions/iteration total, 1 MiB text source/module and require depth 32 (`A/package.rs`), 131,072 bridge calls/VM/ImGui frame, 64 scopes and 196,608 estimated geometry units/VM/ImGui frame (`A/bridge.rs`, `bridge_drawlist.rs:8`). Textures ≤2,048×2,048, ≤128 handles, ≤64 MiB decoded RGBA (`A/textures.rs:34`), input media file ≤16 MiB. Fonts/sound each ≤16 handles, ≤16 MiB/file and ≤64 MiB aggregate (`A/fonts.rs:26`, `A/sound.rs:30..36`). Cache fallback fonts under the same quota. Lua byte/geometry/event/request budgets are independent. No writable arbitrary ImGui ini filename, file-dialog, backend pointer or unsafe draw callback binding.

Lead-approved Round 2 amendment: the bridge call budget is 131,072 per VM per ImGui frame. The pinned Wereoxxs regression with 10,000 scrollback messages uses 30,388 bridge calls (536,000 instructions and 72,024 geometry units); the former 4,096-call limit would reject this unchanged bundled UI. The call budget preserves the ?12 regression gate and remains independent of the instruction and geometry limits.

## 10. Native integration and neutral hooks

All implementation and public Lua registration belong to `fork_ext/addons/` and private `fork_ext/play.rs` children. `host::Snapshot` (`A/host.rs:14..15`) becomes a native core view, with capabilities, presence and generation; Forge adapters stop owning divergent copies/queues. Retain actual source ordering and `ee::*` arithmetic. Never rewrite upstream systems or move packet logic into Lua. A hook in shared code must describe game state/controls/commands only; the shared signature and payload must not name Lua, Forge, addon IDs, callbacks, package manifests or mods.

**Budget policy:** existing addon integration described as four hook lines in the plan is the starting ceiling, not permission to hide new work. Most new reads/actions fit inside existing private child access and frame hooks. The table lists unavoidable new/extended seams and proposed plain-Rust signatures, not implemented in round 1, with **estimates rather than invented exact ceilings**. These are requests for footprint review in the build round; this document does not raise `docs/fork-footprint.json`. The dialog proposal is a small general read-only API exception to hook-only shared edits, requiring explicit lead/footprint review in round 4; it is not silently classified as a hook call. No new hook inside `vi`, renderer, native sound, network parser or arithmetic code is proposed.

| Shared file / call site | Neutral seam signature and payload | Why unavoidable / work / budget ceiling |
|---|---|---|
| `play.rs:2327`, immediately before and after each event match in session pump | `begin_game_event(play:&Play,event:&net::session::Event)->ChangeCapture`, then `end_game_event(play:&Play,capture:ChangeCapture)`; capture stores native stamp and only the relevant prior state/event discriminator. GameStamp={generation,tick,time}. | Two invocation lines: capture before, compare/commit **actual applied** state after match. Preserves multiple transitions within one iteration without re-parsing gameplay logic in Lua. Rejected/malformed updates produce no fabricated changed state. Native early exit must finalize/reset the capture at that exit or revoke it with lifecycle transition; no stale capture leaks. O(relevant event/state payload), bounded copy, no Lua/I/O. Stamp assembly and records belong in adapter. |
| `play.rs:4779`, inside add_chat after accepted logical text is formed, before eight-line truncation | `observe_text(text:&str,color:[f32;4],context:TextContext)`; context contains known native channel/system kind and native commit stamp. | Captures local and server logical lines once. One invocation line (context construction inside child adapter); ≤8 KiB text/event; no formatting/disk I/O. Session tap alone misses locally generated lines; dedup broker does not publish the same packet twice. |
| Existing combat seam at `play.rs:5843` → `F:889` | extend neutral equivalent `observe_hit(attacker:u32,defender:u32,amount:i32,color:Option<[u8;3]>)` to pass **the already parsed rgb** (`P:5830`). | Existing event hook carries three values; one changed invocation line, zero new sites. No fourth-byte read or reinterpretation; O(1). Keep raw sign internal; preserve valid black RGB and nil unless damage_rgb gate + three bytes hold. |
| Existing cooldown seam at `play.rs:2686` → `F:11` | `observe_lockout(display_key:u8,lockout_ms:i32)` retained; adapter resolves ID at **that moment**, before reindex can occur. | No new lines/site; capture every edge. Native whole-record AbilityAdd/update also flows through event observer. No duplicated double delivery. |
| `play.rs:2318`, start of frame, before session pump when game_frame=true | `apply_client_commands(play:&mut Play,walker:&mut Walker,aspect:f32,game_step:bool)`; command/results are neutral typed values in child adapter, never Lua calls. | Existing health_redraw lacks walker and is render-timed (`F:808`); cannot safely perform movement/camera/target actions there. One invocation line using existing aspect from GPU/view or helper; bounded 64 discrete commands/tick. New game command enums/results can be private to client; semantic source sits in fork-owned child. |
| **Live** `viewer.rs:2710..2772`, after macro+touch merge (:2747,:2750), before console.poll (:2754) | `prepare_controls(report:&mut input::Report,connected:&mut bool,stamp:GameStamp)->ControlOrigins`; capture the remapped physical/macro/touch observation **first**, then apply accepted virtual leases/rotation. ControlOrigins sidecar carries Physical/Synthetic origins and separate hold counts through native edge dispatch. | One merge call plus estimated **2–6 dispatch/guard lines** to carry origins around capture/offer paths; exact diff must be measured. Rebind capture (:2726..2734) never receives virtual reports; offer_pad (:2578) must reject synthetic host-only input, not consume it as a player action. Busy/forbidden requests have explicit results. Maintain effective virtual connectivity without hardware. O(16 buttons+4 axes), no input algorithm rewrite. Pregame observation-only hook after :3159 is one additional read call for A input.get; no pregame gameplay injection. Offline shot's :462..483 is only a test-path mirror, never the live implementation. |
| `controls.rs`, just before normal controls update (same caller in walker/viewer) | `merge_control_intent(physical:&Controls,intent:Option<ControlIntent>)->Controls` with normalized movement/look axes, walk flag, lease validity. | Native continuous movement cannot be represented by window key events alone. Prefer call from existing fork-owned viewer/walker child adapter; if common caller lacks access, at most one invocation line there. Keeps game controls algorithm unchanged. No edits inside vi::Walk; O(1). |
| `dialog.rs:10..16,54..75,241..256`, neutral private-state readers | `PrivChat::text(&self)->&[u16]`; `Response::view(&self)->(&[u16],i32,i32)` = current text, native sel, native first; `Response::visit_options(&self,visitor:impl FnMut(i32,i32,&[u16]))` visits each current private Option_ as native index, kind, borrowed label; `MBox::selection(&self)->i32`. Existing MBox.kind/text are public and need no second view struct. | **Revised estimate: 30–45 production lines**, including 4 documented methods, impl/spacing and native option iteration; tests estimated separately at 25–45 lines. No new public view structs, no allocation, no exposing private Option_ or callback into Lua. Native option kinds/indexes from Response::build (:97..119) allow correct More/Back/Close projection. dialog.rs has no current fork budget entry: a new shared-path allowance and this neutral read-only API exception require lead review in round 4 before editing. The old 12-line estimate is withdrawn, not approved. |
| `npcw.rs:426..501`, `bankplay.rs:206..244`, native output consumers | `apply_loot(out:LootOut)->CommandOutcome`, `apply_merchant(out:MerchantOut)->CommandOutcome`, `apply_bank(out:BankOut)->CommandOutcome` as child-adapter methods routing the **same** native output consumer; outcome={applied,sent,rejected(reason)}. | Existing loops embed send/pending behavior. First prefer reuse from the play child with a fork-owned queue consumed by an added generic output-input hook in each loop (one invocation line each, 3 total). If factoring a shared consumer is necessary, retain exact code/ordering, submit separate reviewed seam diff; no duplicated packet writers and no approved rewrite budget. Native loop + queued commands use one consumer and one busy state. |

Shared-site estimate is now **10 invocation lines** (two event-boundary lines, chat, command, live report, pregame read-only report, controls and three transaction consumers), **2–6 extra origin/dispatch guard lines**, one changed combat invocation, and **30–45 dialog accessor lines** (plus 25–45 focused accessor-test lines). Necessary imports/source-sidecar declarations, early-exit finalizers and any offline test-path mirrors are additional measured lines, not hidden in these call-site estimates. This is **not a measured implementation footprint or permission to raise budgets**. Existing hooks are reused where signatures allow; builders must submit exact shared diffs for lead review. The plan's four-line aspiration cannot cover complete events, live controller observation and actions by itself. No owner-only policy question remains; the neutral dialog-reader exception and actual budgets still require lead engineering review.

Avoid new hooks for: player/stats/entity/group reads (private Play fields), item definitions/lists/bank/shop/loot (already native), effects, ability catalog, quests, mastery (read after its existing queued receive), UI busy state (fork adapter derives final window state once per iteration), camera read (walker provided), icon art (frame_library/art_changed already exist), native map (fork-owned module). Engine-side final-state comparison for continuously computed camera/movement or window projection is permitted once/tick/iteration; mods receive events, not a requirement to poll. Packet-sensitive transitions such as chat/combat/cooldown/inventory are captured at mutation/source, never inferred by snapshots.

Map entry points (`images`, `load_image`, waypoint ownership) need **fork-owned** accessor/service methods in map/mod.rs/store.rs; no new shared renderer seam. Window/button fallback may not route a forbidden host action: source metadata on neutral Command is `Physical` or `Synthetic`, with no Lua identity at shared boundary, and host-only commands reject Synthetic. Old weapon/chat extension queues are migrated onto the same broker/consumer; the standalone legacy behavior is retained at its public adapter.

## 11. Forge compatibility mapping

Target unchanged high-level use by Wereoxxs UI, ARPG UI and Avoids Modern UI, plus existing tests/examples. This does not promise that a mod depending on raw addresses or mini_map FFI can run unchanged. Retain reference function/property spellings, colon/dot overload tolerance, return count/order, nil and sentinel conventions; translate only at the adapter boundary. New `eqoa` types/seconds/1-based indexes never leak through Forge getters.

**Choice: hook-named Forge calls control a real native subscription/capture lease.** Install enables that VM's delivery, Uninstall disables it and releases owned capture; IsInstalled reports that state. Acquire/Release owner counts drive those leases. GetHookInfo reports `backend="native"`, actual enabled/suspended/count/dropped information; memory address fields are nil with a diagnostic if inspected, not invented patch sites. Existing mods that simply install/update/subcribe continue working; a request to inspect/modify emulator memory visibly fails. Forge Update drains each VM's cursor once, even if called multiple times/frame; it does not steal another mod's events. IsSendPending is actual broker queued chat, not fixed false.

| Forge module (all 25) / reference anchor | Native eqoa source / round | Compatibility-only shape/index/sentinel rules |
|---|---|---|
| ability (`ability.lua:54..177`) | abilities + cooldowns /2 | Ability.new accepts only opaque native compatibility record tokens obtained from list/bar, no RAM address. Live methods re-resolve ref each call, fixing frozen copies. GetDisplayIndex is native server display key unchanged; spellbook slot getter follows pinned raw value. Cast/cooldown integer seconds, lockout **milliseconds** constant; Scope and size=0x1D8 metadata retained. IsInRange returns bool,distance. |
| ability_list (`:22,35,96,137,651`) | abilities/events /2 | GetAbilityByIndex 1-based, index 0 sentinel, iteration numeric ID order; GetAbilityByDisplayIndex raw reindexable key. OnLockoutStart/End payload fields and owner subscription maps retained. Install/claims operate actual native delivery leases, no MIPS cave. |
| ability_bar (`:161,176,199,229`) | hotbar + items /2, item details /3 | Exactly 3 ability bars, **bar 0..2, slot 0..4**, native carousel selection outside those returns nil (do not clamp); empty slot methods preserve nil. Item/ability lookup adapter follows source tag. GetSlotUITexId preserves actual pinned native art choices; no invented successful texture 0. Slot_size=0x20 metadata. |
| bank (`bank.lua:75,82`) | bank/items /3 | SlotsUsed count; GetItems is dense array `{idx,slot,level,name,amount}`, idx 1-based, slot wear-slot enum unchanged, not ItemRef structure. Contents available even window closed after dump. |
| camera (`camera.lua:6,20,26`) | camera /4 | Vec table {x,y,z}, orbit-facing radians/degrees (north 0, west 90), not body heading. |
| cave (`cave.lua:69,167,237`) | explicitly unsupported /2 diagnostic surface | All seven functions return nil plus unsupported diagnostic; no fake region/cave/site/allocation success. Require returns guarded module so high-level modules can load without a nil require crash. |
| chat (`chat.lua:78,197,594,612`) | chat/events/actions /2 existing send retained, full actions /4 | Preserve MsgType red-channel numeric discriminator, ChatMode string constants, owner+event+fn, OnChatMessage exact legacy field names. GetMessageTypeString is a pure adapter name lookup (supplement below). Update batches logical lines without 8-line loss; current channel preserved by chat_line. SendChatText retains pinned shape backed by real queue and §8's report guard. CaptureChannelTypeFromGameWindow, GetChannelTableInfo and DebugDumpFocusChain are explicitly unsupported: Forge custom-channel memory/config is not native Play.channel. Defined tuple failures below. |
| combat (`combat.lua:433,664,726`) | combat/events /2 | Preserve four damage/heal Dealt/Received names, fields attacker_id/defender_id/amount/seq and flags, signed amount. Convert core RGB8 **only in this adapter** to exact pinned tagged number `0x01000000 + blue*65536 + green*256 + red` (`R/combat.lua:69,452`), nil if core color absent; black becomes 0x01000000. This is not an extra native packet byte. Legacy from_pet retains pinned informational heuristic (actual pet ID, or owner ID with color and a known pet); eqoa's flag stays direct-ID based. Self-heal emits both directions once; real dropped count; ReadForeignCave nil + diagnostic. |
| effects (`effects.lua:10..64`) | effects /2 | max_effects=8; reference slot index **0..7**; All iterator and `{index,icon_ref,name}` with same base; empty after explicit empty channel, unknown nil per getter. |
| entity (`entity.lua:68,205,227,291`) | entities/target /2 | `record` is an opaque native compatibility token, not address. Objects preserve methods/properties/validity, invalidated on generation/incarnation change. HealthPercent is **0..255 raw**, not eqoa fraction. GetTargetId deliberately returns **native_target_id & 0xff**, matching the pinned low-byte read; eqoa Entity.target_id and Player.target_id preserve the **full u32**. The current adapter `A/entity.rs:358..371` still returns its full snapshot target_id: narrowing it is a deliberate Forge-only compatibility correction, never a native ID change. Disposition/name/status constants preserved; unknown→nil/Unknown per reference. |
| entity_list (`entity_list.lua:16..74`) | entities /2 | GetEntityByIndex **0..23** with pinned invalid-index error, GetAllEntities returns 24 slots including empty; slot 0 self synthesized where needed. ID/name lookup may find extra observed records as existing native extension; GetAllEntitiesWithName nil when none (not {}). Tokens must not alias new occupants. |
| gems (`gems.lua:102..112`) | gems /3 | Preserve lookup keys and {type,rarity,stat}; Format corrected to call actual Get (do not preserve crashing lowercase typo), formatter shape unchanged. Document bug fix. |
| group (`group.lua:35,59,117,132`) | group /2 | Member indexes **0-based**; raw HP255 vs fraction getter; unknown HP nil; coordinates {x,y,z} in units, nil when unavailable; count includes self. No fake pointer constructor. Native extra raid list stays eqoa-only. |
| icon (`icon.lua:330,431,479,532`) | icons/textures /2 | Return texture,w,h triple; trim options/owner cache claims retained. GetSurfaceInfo width/height real; palette/format only if genuine decoder metadata exists, otherwise nil + per-field unsupported diagnostics. Texture access must work without an accidental prior UiForge loader install. |
| input (`input.lua:19,49,591,720,1105`) | input/events /2 | Keep active-**low** PS2 mask, raw analog table and pinned normalized shape, Key enum numeric values, owner labels, suppress/text capture rules, PollKeyEvents per-owner cursor, dropped counter. Install hooks are real consumer leases. D-pad rotated=true only when report transformation is applied; no fake success flag. |
| inventory (`inventory.lua:79,109,167,206`) | inventory/items/windows /3 | max_slots=40, compact items; item positions **1-based**. Tab enum **0..3** magic/equipment/tradeskill/misc preserved despite native charged label, actual localized strings; selected slot is inventory position not tab row. GetTabList returns semantic WindowObject/menu descriptor token, no vtable/address. |
| item (`item.lua:152..594`) | items /3 | All getters/live methods/properties and inspect aggregate map to Item record. SLOT/STAT/ATTACK numeric enums unchanged; SLOT_NONE=0xffffffff retained; equip sentinel −1/0xffffffff yields nil equipped position per reference. GetSlotId is **wear_slot**, GetInventoryIndex is **wire inv_index**, not k+1. Class/race masks bit n matches ID n, NoRent=rent==0. Raw record address accepted nowhere. |
| loot (`loot.lua:42..109`) | loot/items /3 | max_slots=40, **1-based** items, closed count 0 and list {}, getters nil; iterator reference shape; never include native Leave All row as an item. |
| merchant (`merchant.lua:57..241`) | merchant/items/windows /3 | Mode menu=0/sell=1/buy=2, Option buy=0/sell=1/exit=2; absent entity 0xffffffff→nil. Stock item/selected index **1-based**, sell selection points inventory; GetSelectedItem nil in sell mode. Greeting/options/button hint exact shapes from reference. |
| player (`player.lua:11..452`) | player/math, mastery /2, CM /4 | Preserve flat Get* names and class/race tables, GetCoordinates/Velocity Vec table, bool getters, numeric IsInGame belongs util. CM nil+diagnostic until supported then real counters/share percent. Forge XP-required helper stays original formula even where eqoa uses native formula; never mix them. |
| quest (`quest.lua:20`) | quests /4 | size=0x100 metadata, new(native token) + GetName, no memory constructor or invented objective fields. |
| quest_log (`quest_log.lua:12,21`) | quests /4 (existing adapter retained in 2) | max_quests=8, **0-based** GetQuestByIndex/Quests, title-only Quest object. Re-resolve on revision; stale token invalid, not another title. |
| ui (`ui.lua:230,462,500`) | hud/icons/imgui /2, icon drawing helpers /4 | All Disable*/Enable* owner-count pairs and compounds retained, DisableUI expands leaf claims; GetHiddenBy sorted legacy owner names scoped by VM. Class/disposition IDs unchanged; GetClassIconTexture triple + Draw helpers render a real ImGui.Image, bool success only on real draw. |
| util (`util.lua:59,103,358,400,618`) | client/player/math/text/log/files/windows/camera /2, windows/map/files /4 | IsInGame returns **0/1 number**; IsStartMenuOpen keeps native **any game window** behavior for shipped overlays. Compass uses camera; GetWorldId actual ID; IsBattleMusicPlaying maps to player.fighting (see supplement). Pure math helpers retained, IsBitZero uses math.is_bit_zero, degree helpers stay degrees. Pointer-typed utf8_to_utf16/utf16_to_utf8 are **unsupported**; all native strings are already UTF-8. Dialogs map semantic state; ListFiles enumerates permitted private/shared-resource views, with private overlays/tombstones. EEmem/pointer/read/write/fill and arbitrary FFI enumeration unsupported. |
| window (`window.lua:134,207,319,618`) | windows + typed menu descriptors /4, existing mode /2 | Mode/ConfirmStyle/InputTag/Classes constants retain pinned IDs/names; high-level active kind, dialogs, hints and menu rows work with semantic native descriptors. Object Read/ReadText only for documented semantic view fields required by high-level adapters; arbitrary offsets, relocation, draw/vtable addresses unsupported. GetMode/IsBusy/IsStartMenuOpen retain native any-window behavior. No fabricated PS2 class tree. |
| UiForge / IGraphicsApi (`UiForge/src/core/core.cpp:672..943`) | events/settings/log/textures/fonts/sound /2, JPEG/private listing /4 | Keep globals/callbacks, Settings/DisableScript/Save/Load/OnEject enum 0..4 and frame-mode entry, LogLevel table. **resources_path is the per-addon overlay over read-only shared mods/resources**, scripts_path/own-package/resources resolves own package overlay (§9). Legacy .cfg/notes reads/writes/removes use those overlays/tombstones; no shared backing writes. IGraphicsApi.CreateTextureFromFile(path) uses the same textures.load service and returns the pinned Texture,w,h triple. Fonts/icons must resolve shared bundled assets; no optional provisioning dependency. Texture failures/nil release/default fonts retain reference conventions. |
| ImGui/ImVec2/ImVec4/enums (`sol_ImGui.h`) | eqoa.imgui /2, explicit additions /4 | Same protected bridge/globals, overloads, tuples and enum numeric IDs. Safe bounded subset in §5; unbound names diagnose with capability rather than imply whole Dear ImGui. |
| ImDrawList (`sol_ImGui.h`, `A/bridge_drawlist.rs:220`) | eqoa.imgui draw lists /2, quad/wrap/path/texture-stack APIs /4 | Colon methods, ImVec2 parameters, packed color, valid Texture handles; frame/window epoch lifetime; AddImageQuad preserves pinned UV/point ordering. Raw pointers/native callbacks unsupported. |

Reference return contracts for Forge wrappers are incorporated from the pinned **public function bodies**, not scout counts; builders must preserve exact arity/defaults for every implemented entry. Round 5 support table enumerates each symbol individually against this mapping, including object properties and nested constants, and explicitly marks each deliberately unsupported entry. Emulator-only metadata (size/stride) is descriptive constant data, never authority to dereference a token. A diagnostic may accompany a legitimate reference-shaped failure, but changing false/nil to unconditional true is forbidden.

The following previously unassigned reference capabilities have explicit homes or exclusions; these rows are normative, not deferred design work:

| Forge function | eqoa equivalent / return policy / round / verified reason |
|---|---|
| `Util.IsBattleMusicPlaying()` | `eqoa.player.get().fighting` /2: legacy bool is fighting==true (false when self creature unavailable). Reference actually reads creature byte +0x263 (`R/util.lua:552`); native `P:8504` reads c.p263 and `viewer.rs:5558` passes it to `sound.rs:482` battle_music. Do **not** replace it with audio output/volume/preview status just because of its misleading legacy name. |
| `Chat.GetMessageTypeString(msg_type:uint)` | `eqoa.chat.channel_name` is the new semantic name helper /2. Forge adapter retains exact red-float **bit-pattern** mapping from `R/chat.lua:78..96`: 0x3F800000→Say, 0x3E7CFCFD→Shout, 0x3F000000→Party, 0x3F008081→Tell, 0x3E800000→Guild, otherwise Unknown Message Type. The native adapter already implements it (`A/chat.rs:142,177`). No raw memory needed. |
| `Chat.CaptureChannelTypeFromGameWindow()` | **unsupported** /2 diagnostic surface; returns `nil,"unsupported: native client has no Forge custom-channel window configuration",nil`. The reference reads the custom 0x6c per-window config array and selected slot (`R/chat.lua:667..738`), not ordinary typed-chat channel. Native WndChat.mode (`chat.rs:316`) and Play.channel (`P:4613`) cannot supply those bytes/slot faithfully. Remove current `(0,nil,0)` placeholder (`A/chat.rs:365`); use eqoa.chat.get_channel for ordinary channel only. |
| `Chat.GetChannelTableInfo()` / `Chat.DebugDumpFocusChain()` | **unsupported** /2 diagnostic surface: return `nil,"unsupported: native client has no Forge channel-pointer table/focus-chain addresses"` (two returns). Reference `R/chat.lua:745..796` introspects pointers/byte0/byte1 of a 256-entry table and vtables; no corresponding native semantic data. Never return a fabricated empty table (`A/chat.rs:369..376`). |
| `UiForge.IGraphicsApi.CreateTextureFromFile(path:string)` | Same core as eqoa.textures.load /2 PNG, /4 JPEG; convert TextureInfo to pinned `texture,width,height` returns; failure nil with diagnostic. Actual reference binding `R/UiForge/src/core/core.cpp:938..943`, existing native alias `A/textures.rs:463..473`. Uses identical private/overlay/shared-read resolver. |
| `Util.utf8_to_utf16(text)` / `Util.utf16_to_utf8(pointer)` | **unsupported** /2 diagnostic surface: `nil,"unsupported: pointer-based UTF conversion is unavailable; native API strings are UTF-8"`. These reference functions allocate/accept FFI wchar buffers (`R/util.lua:247..305`); they are not safe Lua-string-only helpers. No pointer replacement or code-unit-array promise. Native host performs UTF decoding at each string boundary. |
| `Util.IsBitZero(value:uint,mask:uint)` | eqoa.math.is_bit_zero /2, same `(value & mask)==0` (`R/util.lua:377`); mask is a bit mask, not bit position. Existing global/require bit remains separate with its actual eight-function BitOp surface (`A/bit.rs:54`). |

### Existing Enhanced extensions: permanent compatibility aliases

Do not rename/remove them. `eqoa.WeaponSets` exposes `IsAvailable()->bool`, `GetCount()->uint` (0 unavailable), `GetSelected()->int?`, `GetIcon(index:int)->Texture?,width?,height?`, `GetBadge(index:int)->same`, `GetName(index:int)->string?`, `IsEmpty(index:int)->bool`, `Use(index:int)->bool`; **index 0-based** across native gear::ENTRIES. Exact current adapter `A/weapon_sets.rs:75..180`, consumer `F:111,803`. Use true means accepted into bounded request queue, not server confirmed; false on unavailable/invalid/full. Back it with the shared action queue retaining this old return shape. `eqoa.ChatHistory()->{active:bool,steps:int}` (`A/chat_history.rs:19..39`) consumes accumulated signed steps for that VM, clamp existing MAX_STEPS_PER_CALL, active false reports 0 and advances baseline. New chat.history_scroll is event form; do not double-consume legacy caller cursor. These aliases do not impose their casing/base on new modules.

## 12. Delivery and verification gates

| Round | Deliverable and acceptance |
|---|---|
| 1 Design | This spec, owner summary and design report; source review only, no game code/builds. Independent review belongs to lead, not this designer. |
| 2 Core | client/player/entities/target/group/abilities/cooldowns/effects/chat/combat/input/hotbar/HUD, base icons/text/media/files read-write/settings/log/math, event broker/lifecycle/capabilities/diagnostics/sandbox, stable aliases and Forge rewiring. Preserve existing chat send/weapon actions and quest-log adapter. Implement real effects/pad or visibly report unsupported until completed; no hidden empty defaults. Add native hearts example and getting-started. |
| 3 Items | full inventory/items/bank/loot/merchant/gems reads, tabs/selection and events, Forge adapters, bag viewer + loot log; empty/closed/zone/compaction cases. No new transaction automation yet. |
| 4 Rest | quests/objects/events, camera, typed windows/dialogs/busy, map/world/image/waypoint, mastery counters/share/skills, class/disposition textures and draw helpers, JPEG/private listing, tooltip/mouse/style/quad/wrap additions, all game actions/arbitration/neutral seams, examples. |
| 5 Finish | Each public native and Forge symbol audited real or visibly unsupported; support table and author docs; three bundled UIs unchanged in live play; owner release approval freezes eqoa/1. Old docs branch retired only with owner's OK. |

Build-round checks: existing focused addon suite, scratch EQOA_CONFIG, shared build lock, wasm check, footprint checker against reviewed base, and required release build under lead coordination. This design round runs none of those builds/tests. Future tests must traverse production snapshots/event sources/action consumers, not only injected standalone fixtures. Include negative controls for default-only effects/controller, >8 chat burst, multiple same-frame cooldown edges/reindex, unknown health, leader transfer, full u32 target ID, quest/list compaction, stale generation refs, loot close/reopen, closed bank contents, merchant replacement/pending/refusal, no extra sends from high render FPS, two-mod control contention, disable/focus-loss releases and sandbox path/require/media escapes. Mock a void call that silently refuses and prove it cannot report sent. Count all actual fixture skips; passing injected fixtures are not live parity evidence.

**Round-2 bundled-resource regression cases (unchanged Lua files):**

- Wereoxxs' real shared fonts `resources/fonts/Cinzel/static/Cinzel-Regular.ttf`, `resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf`, `resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf` (`bundled-mods/wereoxxs_ui/wereoxxs_ui.lua:159..161`), and `UiForge.resources_path .. "\\gear-icon.png"` (:759), must load real shared art rather than silently falling back. Exercise both bare resource names and issued-prefix aliases in media loaders.
- Modern UI current notes path `UiForge.scripts_path .. "\\avoids_modern_ui\\resources\\quest_log_notes.cfg"` (`bundled-mods/avoids_modern_ui/modules/avoids_persist.lua:19`, `modules/avoids_quest_log.lua:108..109,147`) must write/read only its private package-resource overlay, survive Save/restart, and not change either bundled/shared backing.
- Legacy files under resources_path must be readable once, with private migration/tombstone behavior: `modern_ui_mods_settings.cfg`, `ability_bar_settings.cfg`, `chat_log_settings.cfg`, `effects_settings.cfg`, `exp_bars/exp_bars_settings.cfg`, `group_frames_settings.cfg`, `retro_health_hearts/health_bar_settings.cfg`, `mana_pool/mana_bar_settings.cfg`, `pet_frame_settings.cfg`, `quest_log_settings.cfg`, and old `quest_log_notes.cfg`. Sources: `avoids_modern_ui.lua:119..136` and modules' LEGACY_SETTINGS_FILE/LEGACY_NOTES_FILE definitions. Test nested path parents and both slash spellings.
- `avoids_persist.MigrateLegacyFile` (`modules/avoids_persist.lua:81..105`) reads then pcall(os.remove); after migration/profile save/restart, this addon must not reread the shared old config, and another addon must still read its own fallback. Notes migration must write the new own-package notes before tombstoning the old shared alias (`modules/avoids_quest_log.lua:179..192`). Simulated overlay/close/tombstone I/O failure must remain visible, never delete shared backing or claim successful persistence.
- Negative controls: overlay shadows shared for the owning addon only; two mods writing the same virtual resource name stay isolated; os.rename/remove never touch shared backing; other-package resources and private areas, traversal/reparse paths and raw shared-root absolute paths outside issued aliases are refused. ARPG UI plus both other bundled UIs must still initialize/Save/Load/disable/reload using the same scoped media/file resolver. Native behavior of shipped scripts is exercised, not a copied fixture-only substitute.

Other focused review regression gates: exactly one drawing path per entryMode, no pre-world HUD hide claim, union hidden_by for compass/hotbar/all, all normative native mode IDs including 7/30/36/60, RGB black/short/missing/feature-disabled trailers and exact Forge tagging, all 1-based menu/tab/slot conversions, no synthetic capture/update-offer side effects or dead virtual-device path, and #bug/BUG/report-form refusal for both native and Forge sends without invoking send_bug_note.

After source/runtime gates, owner tests: unchanged Wereoxxs/ARPG/Modern UI bars, effects, compass, chat/send/history, cooldown and combat, item/bank/vendor/loot events, native waypoint, action examples and emergency disable. No new release or branch integration authorized by this document. FORK-UPDATE-NOTES changes occur with player-visible implementation, not with this design-only commit.

## 13. Limits

The client cannot supply data it never received: whole-world live spawn lists, unobserved entity HP/disposition/targets, exact effect duration/stacks/caster, full active quest objectives/progress, or exact server cooldown time remaining. Unknown is nil with stated availability, not guessed from catalog entries, elapsed render frames or fixed zeros. Bank contents exist closed after dump; merchant/loot are session data; don't generalize all three as open-window-only.

There is no emulated EE RAM, process memory API, FFI, code cave, vtable/draw-address scanner, native pointer userdata or executable memory patcher. Forge's mini_map rasterizer depends on those and is excluded; native calibrated map images/waypoints are the supported replacement. Pointer-based Util.utf8_to_utf16/utf16_to_utf8, custom-channel CaptureChannelTypeFromGameWindow, GetChannelTableInfo and DebugDumpFocusChain have no safe native equivalent for the pointer/config payloads they promise; their explicit diagnostic failures are in §11. Ordinary channel names, UTF-8 game strings, bit-mask checks, combat flag and file textures remain provided there. No obstacle-aware navigation or automatic route planner is implied by allowing botting. Sandbox forbids arbitrary networking/process launches and file access outside the addon's private mounts **except the designated read-only shared bundled resource/support mounts** (§9). Browser addons are unsupported until a separate runtime is built; native API capability presence is not a wasm-support claim. Sound is supported only where its real backend exists.

Drawing support is the explicit bounded subset in §5, not every pinned ImGui binding. Reference helpers that genuinely require raw storage inspection remain visibly unsupported; this limitation does not justify withholding high-level game data that the native structures already contain.

## Open questions

None. The owner has already chosen naming, stability timing, Forge compatibility, automation and sandbox scope. Technical implementation checks (native reply correlation, seam footprint review, actual platform/media availability) are assigned above with defined fallback/error behavior; they are not owner policy decisions.
