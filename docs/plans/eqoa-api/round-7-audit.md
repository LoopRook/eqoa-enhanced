# Round 7 API audit: `eqoa.*` and `frontiers_forge.*`

Read-only audit of the round 6 API worktree (`feat/eqoa-api-r6`, tip `2e4fb5fe`), branch
`docs/round-7-audit`, 2026-11-17. No build or test was run. Everything below is from source
inspection. Where the source could not settle a question I say so in a UNCLEAR note (end of §4)
rather than guessing.

Verdicts used:

| Verdict | Meaning |
|---|---|
| REAL | Does what the spec says, over the native data/handler it names. |
| DIAGNOSTIC | Refuses visibly: `nil,Error` with `not_yet`/`unsupported`, or a logged diagnostic. |
| FAKE SUCCESS | Returns ok/true/a handle/plausible data while doing nothing, with no diagnostic. |
| PARTIAL | Works for some inputs and is silently empty/wrong/absent for others (named per row). |
| UNCLEAR | Source reading did not settle it; the row says what is unknown. |

## 1. Method and counts

Shadow map of the surface:

- **Lua registration.** Every `eqoa.*` function is registered by `native::bind`/`bind_fn`
  (`crates/eqoa/src/fork_ext/addons/native.rs:712,775`), which writes into the `eqoa_modules`
  registry, records a descriptor (`native.rs:798-840`) and wraps errors into the `nil,Error`
  contract (`native.rs:384-397`). I extracted every call site mechanically across
  `crates/eqoa/src/fork_ext/**`: about **140 literal registrations** plus **12 table-driven
  loops** (`action_broker.rs:3117,3385`; `file_io.rs:538`; `icons.rs:1394`; `native_events.rs:263`;
  `native_items.rs:610`; `native_media.rs:315`; `native_settings.rs:99`;
  `native_state.rs:151,181,670`; `window_actions.rs:188,217,326`), each resolved by hand.
  The raw match count also contains test-harness matches and non-Lua `bind` methods, so the
  module totals in §2 are the authority.
- **Descriptor seed.** The normative-as-built index is `native.rs:65-144`; it pre-creates a
  `not_yet` stub for every name in it, so a spec symbol with no registration still answers
  `nil,Error{code="not_yet"}` instead of `attempt to call a nil value`.
- **Record fields.** `install_fields` (`native.rs:1128-1282`), `native_items.rs:575-604`,
  `native_r6a.rs:392-426` and `native_events.rs:129-212` declare every record field as
  `supported`; these are capability claims with no runtime gate, so they are audited below as
  claims.
- **Forge.** `register_host_module` (22 calls) builds the module tables; `annotate_host`
  (`native.rs:266-361`) then declares every module `partial` and **every member `supported`**
  (`native.rs:302-307`) without looking at what the member does. The six deliberately
  unsupported members are re-described afterwards (`native.rs:310-325`).
- **Imgui.** `native_media::imgui` (`native_media.rs:8-73`) describes `eqoa.imgui.<Name>` and
  `ImGui.<Name>` for every function actually bound by `bridge.rs`/`bridge_widgets.rs`, marks a
  named not-yet list, and diagnoses unbound members through the `__index` metatable
  (`native_media.rs:74-87`).

Counts (callables only; values/constants excluded):

| Group | Symbols |
|---|---|
| `eqoa.*` module functions | 184 across 40 modules |
| `eqoa.actions.*` | 49 of those |
| Legacy `eqoa.WeaponSets.*` | 8 (+ table) |
| Legacy `eqoa.ChatHistory()` | 1 |
| Root values | 2 (`eqoa.api`, `eqoa.revision`) |
| `frontiers_forge.*` modules | 22 registered + `util` (require-only) = 23 of the spec's 25 |
| `frontiers_forge.*` members | about 460 `set("Name", …)` entries; the client derives the exact list at runtime |
| ImGui bridge members | every function of the bridge table (≈110), described in pairs |
| `bit.*` | 8 global functions, **no descriptors at all** |

Verdict totals over the 219 table rows: **REAL 194, DIAGNOSTIC 18, PARTIAL 5, FAKE SUCCESS 2,
UNCLEAR 0** (three namespaces carry a UNCLEAR note instead of a row; see the end of §4).

## 2. Namespace tables

Column `cap` is the `eqoa.capabilities` answer for the symbol: `sup` = supported, `ny` = not_yet,
`unsup` = unsupported, `unk` = unknown, `part` = partial (module only). The round in parentheses
is the descriptor's `round` value.

### 2.1 Root, legacy aliases, capabilities

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.api` | `native.rs:848` | §2 | sup (2) | REAL | Set to `"eqoa/1"`; `eqoa.revision` is 1 at `native.rs:849`. |
| `eqoa.capabilities.get(path)` | `native.rs:883-894` | §2 | sup (2) | REAL | Returns the descriptor record; unknown paths give `status="unknown"` silently (`native.rs:185-191`). |
| `eqoa.capabilities.has(path)` | `native.rs:895-906` | §2 | sup (2) | REAL | True only for exact `supported`, as specified. |
| `eqoa.capabilities.list(prefix)` | `native.rs:907-930` | §2 | sup (2) | PARTIAL | Sorted and prefix-filtered, but only contains what `describe` saw: `bit.*` is absent (no `bit.` string exists in the tree), `frontiers_forge.util` is absent (it is a `require` resolver entry, `host.rs:546`, not a host module), and `frontiers_forge.quest`/`.cave` are absent (§3). |
| `eqoa.ChatHistory()` | `chat_history.rs:20` | §11 | sup (2) | REAL | Returns the VM's accumulated `{active,steps}` and advances the baseline (`chat_history.rs` install). |
| `eqoa.WeaponSets.IsAvailable/GetCount/GetSelected/GetName/IsEmpty` | `weapon_sets.rs:93,98,109,140,154` | §11 | sup (2) | REAL | Read the native gear table/snapshot, 0-based per the legacy contract. |
| `eqoa.WeaponSets.GetIcon/GetBadge(index)` | `weapon_sets.rs:115` | §11 | sup (2) | REAL | Return `texture,w,h` from the icon cache for the entry index. |
| `eqoa.WeaponSets.Use(index)` | `weapon_sets.rs:166` | §11 | sup (2) | REAL | Queues the shared weapon-set request and returns true only when accepted into the bounded queue. |

### 2.2 `eqoa.client`, `eqoa.player`, `eqoa.entities`, `eqoa.target`, `eqoa.group`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.client.get()` | `native_state.rs:150` | §4 | sup (2) | REAL | Reads the frozen core (`in_world/phase/generation/tick/frame/time/world_id`). The placeholder at `native.rs:931` is replaced by this binding. |
| `eqoa.client.display()` | `native_media.rs:92` | §5 | sup (2) | REAL | Real ImGui IO size + render scale; `unavailable` without a context. |
| `eqoa.player.get()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Reads `view.player`; `not_in_world` otherwise. Placeholder `native.rs:932` is replaced. |
| `eqoa.player.stats()` | `native_state.rs:761` | §5 | sup (3) | REAL | Reads the native detailed stats snapshot, `unavailable` before it arrives. |
| `eqoa.player.experience_for_level(level)` | `native_state.rs:510` | §4 | sup (2) | REAL | Pure `hud::exp_for_level`, nil outside 1..59. |
| `eqoa.entities.list()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Array of the current entity records. |
| `eqoa.entities.get(ref)` | `native_state.rs:187-193` | §4 | sup (2) | REAL | Resolves by generation+incarnation; `stale_handle` when gone. |
| `eqoa.entities.find(id)` | `native_state.rs:195-216` | §4 | sup (2) | REAL | ID lookup, `nil` for a missing record (ordinary absence). |
| `eqoa.entities.named(name,exact)` | `native_state.rs:330-374` | §4 | sup (2) | REAL | Case-insensitive filter over the live list. |
| `eqoa.entities.distance(a,b)` | `native_state.rs:623` | §4/§5 | sup (2) | REAL | `ee::*` distance of two Vec3 tables. |
| `eqoa.entities.dots()` | `native_state.rs:295-329` | §4 (body) | sup (4) | REAL | Column arrays built from this frame's native minimap dots. |
| `eqoa.entities.pick_dot(q)` | `native_state.rs:217-294` | §4 (body) | sup (4) | REAL | Projects this frame's dots and returns the nearest inside the circle. |
| `eqoa.target.get()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Finds the entity matching `player.target_id`. |
| `eqoa.group.get()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Group snapshot, or unavailable state pre-world. |

### 2.3 `eqoa.abilities`, `eqoa.hotbar`, `eqoa.cooldowns`, `eqoa.effects`, `eqoa.combat`, `eqoa.chat`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.abilities.list()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Ability snapshot array. |
| `eqoa.abilities.get(ref)` | `native_state.rs:187-193` | §4 | sup (2) | REAL | Handle resolution with `stale_handle`. |
| `eqoa.abilities.find(id)` | `native_state.rs:376-400` | §4 | sup (2) | REAL | Canonical decimal-string ID lookup. |
| `eqoa.abilities.in_range(ref,from,to)` | `native_state.rs:754` | §4 | sup (2) | REAL | Range result from the ability's native range and `ee` distance. |
| `eqoa.hotbar.list()` | `native_state.rs:151-180` | §4 | sup (2) | REAL | Bar/slot snapshot with resolved item references. |
| `eqoa.cooldowns.get(ref)` | `native_state.rs:187-193` | §4 | sup (2) | REAL | Lockout plus edge timestamps from the cooldown observer. |
| `eqoa.effects.list()` | `native_state.rs:402-430` | §4 | sup (2) | REAL | `unavailable` until channel 0x43 arrives; never invents an empty list. |
| `eqoa.combat.get_counters()` | `native_events.rs:362` | §4 | sup (2) | REAL | Real received/dropped counters from the event bus. |
| `eqoa.chat.history(limit,before)` | `native_state.rs:456-508` | §4 | sup (2) | REAL | Paged over the native chat ring, newest page ordered. |
| `eqoa.chat.get_channel()` | `native_state.rs:432` | §4 | sup (2) | REAL | Current typed-chat channel from the core view. |
| `eqoa.chat.channel_name(ch)` | `native_state.rs:440` | §4 | sup (2) | REAL | Pure name map, `"Unknown Message Type"` fallback. |

### 2.4 `eqoa.events`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.events.on(name,cb)` | `native_events.rs:263-301` | §3 | sup (2) | REAL | Registers into the per-VM bus (128 cap); unknown names return the descriptor's status; `render` in frame mode is `invalid_argument` (`native_events.rs:279-286`). |
| `eqoa.events.once` | `native_events.rs:263-301` | §3 | sup (2) | REAL | Same path with `once=true`, removed before first call. |
| `eqoa.events.off(token)` | `native_events.rs:303-321` | §3 | sup (2) | REAL | Token-kind and VM check, true/false per spec. |
| `eqoa.events.after(seconds,cb)` | `native_events.rs:323-350` | §3 | sup (2) | REAL | Monotonic deadline in the subscription list, quota on 128. |

### 2.5 `eqoa.hud`, `eqoa.icons`, `eqoa.imgui`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.hud.hide(element)` | `visibility.rs:642-710` | §5 | sup (2) | REAL | Inserts an owner claim into the visibility intent and returns a token. The `native.rs:945` placeholder is replaced whenever a visibility lease is installed (`visibility.rs:238-253`, `host.rs:503`). |
| `eqoa.hud.show(token)` | `visibility.rs:712-747` | §5 | sup (2) | REAL | Removes the claim, returns `old_generation && alive`. |
| `eqoa.hud.hidden_by(element)` | `visibility.rs:749-780` | §5 | sup (2) | REAL | Sorted owner IDs for the element's leaves. **The `native.rs:958` placeholder returns `{}` silently**; see §4 fix 2. |
| `eqoa.hud.frame{width,height}` | `native_state.rs:815-833` | §5 | sup (3) | REAL | Pure layout from `hud_layout`, defaults to the window size. |
| `eqoa.hud.layout{width,height}` | `native_state.rs:835-853` | §5 | sup (3) | REAL | Rectangles from the native layout. |
| `eqoa.hud.visible(element)` | `native_state.rs:855-869` | §5 | sup (3) | REAL | False pre-world, native leaf state otherwise. |
| `eqoa.hud.text{...}` | `native_state.rs:886-972` | §5 | sup (3) | REAL | Queues a text command into the HUD pass; `limit_exceeded` on overflow. |
| `eqoa.hud.gauge{...}` | `native_state.rs:974-1038` | §5 | sup (3) | REAL | Same queue, gauge command. |
| `eqoa.hud.image{...}` | `native_state.rs:1039-1129` | §5 | sup (3) | REAL | Real `ImDrawList_AddImage[ Rounded]` on the foreground list (`hud_textures.rs:106-143`). |
| `eqoa.hud.panel{...}` | `native_state.rs:1130-1191` | §5 | sup (3) | REAL | Queues the panel command. |
| `eqoa.icons.resource(hash,opts)` | `icons.rs:1394-1499` | §5 | sup (2) | REAL | Loads/returns `texture,width,height` from the live icon library; `not_found` when absent. |
| `eqoa.icons.ui(tex_id)` | `icons.rs:1394-1499` | §5 | sup (2) | REAL | UI-sheet texture path of the same adapter. |
| `eqoa.icons.info(hash)` | `icons.rs:1501-1530` | §5 | sup (2) | PARTIAL | `width/height` real; `source_format` is hard-coded `null` (`icons.rs:1514-1518`) and the field is still declared `supported` (`native.rs:1231`), with no per-field diagnostic. |
| `eqoa.icons.class(class_id)` | `icons.rs:1532-1575` | §5 | sup (4) | REAL | Class icon texture from the native art cache. |
| `eqoa.icons.disposition(d)` | `icons.rs:1577-1633` | §5 | sup (4) | REAL | Disposition face texture from the same cache. |
| `eqoa.imgui.vec2/vec4` | `native_media.rs:11-16` | §5 | sup (2) | REAL | Aliases of `ImVec2.new`/`ImVec4.new`. |
| `ImGui.<Name>` / `eqoa.imgui.<Name>` | `native_media.rs:17-24` | §5 | sup (2) / ny (4) | REAL | Every bound bridge function is real; unbound members diagnose `unknown_symbol` (`native_media.rs:74-87`); the named gaps return `not_yet` stubs (`native_media.rs:32-73`). |

### 2.6 `eqoa.math`, `eqoa.text`, `eqoa.log`, `eqoa.files`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.math.bearing` | `native_state.rs:729` | §5 | sup (2) | REAL | `ee::libm` atan2 of the coordinate delta. |
| `eqoa.math.degrees/radians/normalize` | `native_state.rs:670-692` | §5 | sup (2) | REAL | Pure conversions, `invalid_argument` on non-finite. |
| `eqoa.math.signed_angle` | `native_state.rs:709-728` | §5 | sup (2) | REAL | Wrapped difference in `(-π,π]`. |
| `eqoa.math.facing(p,h,d,tol)` | `native_state.rs:637-668` | §5 | sup (2) | REAL | Bearing vs heading inside tolerance, `out_of_range` for tolerance. |
| `eqoa.math.is_bit_zero(v,m)` | `native_state.rs:693-707` | §5 | sup (2) | REAL | `(v & m) == 0` on u32s. |
| `eqoa.text.localized(id)` | `native_state.rs:531-562` | §5 | sup (2) | REAL | Reads the native word table, `unavailable` before load, nil for `(null)`. |
| `eqoa.log.write(level,msg)` | `native.rs:1035-1070` | §5 | sup (2) | REAL | Bounded 64-message queue, `quota` when full. |
| `eqoa.files.read(path)` | `file_io.rs:538-620` | §5 | sup (2) | REAL | Mount resolver + 1 MiB read; absolute/drive paths denied. |
| `eqoa.files.write(path,bytes)` | `file_io.rs:538-620` | §5 | sup (2) | REAL | Private-overlay write through the same resolver. |
| `eqoa.files.remove(path)` | `file_io.rs:538-620` | §5/§9 | sup (2) | REAL | Tombstone semantics in the overlay, never the shared backing. |
| `eqoa.files.list(dir,glob)` | **not registered** | §5:420, index /4 | ny (4) | DIAGNOSTIC | Seed stub at `native.rs:85`; `file_io.rs:538` registers only read/write/remove. Returns `nil,not_yet`. |
| `eqoa.files.load_json(path)` | `async_json.rs:322` | §5/Open Q | sup (4) | REAL | Queues the native JSON worker, completion through `resource.ready/failed{kind="json"}`. |
| `eqoa.files.save_json_async(path,v)` | `json_save.rs:416` | §5/Open Q | sup (4) | REAL | Snapshot + worker + atomic replace, coalescing per path. |
| `eqoa.files.import{...}` | `player_files.rs:466` | §Open Q | sup (4) | REAL | Permission + activation gate, OS picker off-thread, `resource.ready{kind="import"}`. |
| `eqoa.files.export(path,name)` | `player_files.rs:527` | §Open Q | sup (4) | REAL | Same gate, Desktop copy with collision suffixes. |
| `eqoa.files.null` | `async_json.rs:320` | §Open Q | sup (4) | REAL | The JSON null sentinel. |

### 2.7 `eqoa.fonts`, `eqoa.sound`, `eqoa.textures`, `eqoa.settings`, `eqoa.binds`, `eqoa.options`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.fonts.load(path,size)` | `fonts.rs:423` | §5 | sup (2) | REAL | Queues the atlas; `FontInfo.status` reports pending/ready/fallback. |
| `eqoa.fonts.default()` | `fonts.rs:481` | §5 | sup (2) | REAL | Immortal default handle; `unavailable` without context. |
| `eqoa.fonts.release(font)` | `fonts.rs:503` | §5 | sup (2) | REAL | Marks the handle dead; nil is idempotent true. |
| `eqoa.fonts.game()` | **not registered** | §2 index /3 | unk | DIAGNOSTIC (absent) | No `eqoa.fonts.game` binding or descriptor; the call is a Lua "attempt to call a nil value", not a diagnostic. §3 mismatch. |
| `eqoa.sound.load/play/stop/set_volume/is_playing/release` | `native_media.rs:262-428` | §5 | sup (2) | REAL (Windows) / DIAGNOSTIC (other OS) | Real backend calls on Windows; elsewhere every name is described `unsupported` (`native_media.rs:430-446`) and `bind` also refuses file/sound on non-Windows (`native.rs:716-719`). |
| `eqoa.textures.load(path)` | `native_media.rs:105` | §5 | sup (2) | REAL | Private/overlay resolver → decode → owned handle. |
| `eqoa.textures.rgba(bytes,w,h)` | `native_media.rs:141-177` | §5 | sup (2) | REAL | Byte-count-validated upload. |
| `eqoa.textures.release(t)` | `native_media.rs:180-215` | §5 | sup (2) | REAL | VM ownership check, marks dead. |
| `eqoa.settings.load()` | `native_settings.rs:34` | §5 | sup (2) | REAL | Returns the stored JSON value (nil when none). |
| `eqoa.settings.save(v)` | `native_settings.rs:49` | §5 | sup (2) | REAL | Validates and writes through the settings service. |
| `eqoa.settings.on_save/on_load(cb)` | `native_settings.rs:99-116` | §5 | sup (2) | REAL | Registers the VM callbacks invoked by the service. |
| `eqoa.binds.register(id,key,desc)` | `native_state.rs:1284-1296` | §5 | sup (3) | REAL | Adds a row to the Controls page; returns the native accept result. |
| `eqoa.binds.get(id)` | `native_state.rs:1297-1305` | §5 | sup (3) | REAL | Current key for the addon's binding. |
| `eqoa.options.toggle{id,label,default}` | `native_state.rs:1216-1255` | §5 | sup (3) | REAL | Registers a manager row, returns `{id,label,default,value,item_id}`. |
| `eqoa.options.get(id,owner)` | `native_state.rs:1256-1270` | §5 | sup (3) | REAL | Reads the persisted toggle (foreign owner ANDed with that addon being enabled). |
| `eqoa.options.set(id,bool)` | `native_state.rs:1271-1283` | §5 | sup (3) | REAL | Persists; `io_error` on failure. |
| `eqoa.options.button(spec,cb)` | `option_buttons.rs:49` | §Open Q | sup (3) | REAL | Registers the action row and queues the callback on a click. |

### 2.8 `eqoa.input`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.input.get()` | `native_state.rs:564` | §4 | sup (2) | REAL | `input::native_get` (focused/pad/text_capture); placeholder `native.rs:964` replaced. |
| `eqoa.input.claim(key,suppress)` | `native_input.rs:42-102` | §4 | sup (2) | REAL | Inserts a real capture lease with a token. |
| `eqoa.input.release(token)` | `native_input.rs:103-132` | §4 | sup (2) | REAL | Token-kind check, removes the lease. |
| `eqoa.input.capture_text(bool)` | `native_input.rs:133-158` | §4 | sup (2) | REAL | Sets the native text-capture lease and neutralises merged pad input. |
| `eqoa.input.rotate_dpad(bool)` | `native_input.rs:159-190` | §4 | sup (2) | REAL | Applies the report rotation (`native_input.rs:186` succeeds only when the transform is active). |
| `eqoa.input.bind{id,...}` | `native_state.rs:1307-1385` | §4 | sup (3) | REAL | Registers the binding and wraps the callback behind the mod-key/modifier check. |
| `eqoa.input.pointer()` | `pointer_api.rs:125` | §5/§7 | sup (4) | REAL | Physical window pixels from the last captured pointer. |
| `eqoa.input.claim_pointer(rect)` | `pointer_api.rs:145-202` | §5/§7 | sup (4) | REAL | Per-VM, per-frame claim list, 32 cap, expires. |
| `eqoa.input.minimap_wheel(steps)` | `pointer_api.rs:203` | §7 | sup (4) | REAL | Forwards to the native minimap zoom when the point is claimed. |

### 2.9 `eqoa.inventory`, `eqoa.bank`, `eqoa.loot`, `eqoa.merchant`, `eqoa.items`, `eqoa.gems`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.inventory.get()` | `native_items.rs:610-622` | §6 | sup (3) | REAL | Container record from the frozen view; `nil,nil` when closed; revision-cached table. |
| `eqoa.bank.get()` | `eqoa.inventory.get` | §6 | sup (3) | REAL | Same path, bank slot; contents survive a closed window (native dump). |
| `eqoa.loot.get()` | `eqoa.inventory.get` | §6 | sup (3) | REAL | Same path, corpse list. |
| `eqoa.merchant.get()` | `eqoa.inventory.get` | §6 | sup (3) | REAL | Same path, merchant session. |
| `eqoa.items.get(ref)` | `native_items.rs:623-628` | §6 | sup (3) | REAL | Resolves the handle to the live stack (`stale_handle` when gone). |
| `eqoa.items.stat(ref,id)` | `native_items.rs:629-648` | §6 | sup (3) | REAL | Looks up the stat in the item's array, 0 when absent (documented item shape). |
| `eqoa.items.can_use(ref,class,race,level)` | `native_items.rs:649-700` | §6 | sup (3) | REAL | Uses the native class/race/level masks and reasons. |
| `eqoa.items.slot_info(id)` | `native_items.rs:701-718` | §6 | sup (3) | REAL | `{id,name,order}` from the native wear-slot table. |
| `eqoa.items.attack_name(id)` | `native_items.rs:719-736` | §6 | sup (3) | REAL | Localized attack word, `unavailable` before words load. |
| `eqoa.items.stat_name(id)` | `native_items.rs:737-750` | §6 | sup (3) | REAL | Localized stat word, `unavailable` before words load. |
| `eqoa.merchant.price(ref,dir,qty)` | `native_items.rs:751-795` | §6 | sup (3) | REAL | Native buy/sell rates, `wrong_window` without a merchant. |
| `eqoa.gems.get(name)` | `gems.rs:117-134` | §6 | sup (3) | REAL | Static native gem table; nil for a non-gem. |
| `eqoa.gems.is_gem(name)` | `gems.rs:135-146` | §6 | sup (3) | REAL | Table membership. |
| `eqoa.gems.format(name,amt,fmt?)` | `gems.rs:147-175` | §6 | sup (3) | REAL | Formatted text from the table; formatter errors surface as `failed`. |

### 2.10 `eqoa.quests`, `eqoa.camera`, `eqoa.windows`, `eqoa.mastery`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.quests.list()` | `native_r6a.rs:100-109` | §7 | sup (4) | REAL | `world_read` of the quest log; `not_in_world` otherwise. |
| `eqoa.quests.get(ref)` | `native_r6a.rs:110-140` | §7 | sup (4) | REAL | Resolves by ref key+incarnation; `stale_handle`. |
| `eqoa.camera.get()` | `native_r6a.rs:209-236` | §7 | sup (4) | REAL | Walker camera snapshot; `unavailable` until a walker frame. |
| `eqoa.camera.compass()` | `native_r6a.rs:237-264` | §7 | sup (4) | REAL | Native compass direction; `unavailable` until known. |
| `eqoa.windows.get()` | `native_r6a.rs:267-275` | §7 | sup (4) | REAL | Window/busy snapshot; works pre-world as an empty `none` state. |
| `eqoa.windows.find(kind)` | `native_r6a.rs:276-308` | §7 | sup (4) | REAL | First visible window of the kind, nil when none. |
| `eqoa.windows.get_window(ref)` | `native_r6a.rs:309-329` | §7 | sup (4) | REAL | Ref resolution incl. incarnation; `stale_handle`. |
| `eqoa.windows.format_hint(button,label)` | `native_r6a.rs:330-367` | §7 | sup (4) | REAL | Pure glyph prefix from `binds::BUTTONS`. |
| `eqoa.windows.parse_hint(text)` | `native_r6a.rs:368-390` | §7 | sup (4) | REAL | Parses glyph-prefixed hint text into `{button,label}` rows. |
| `eqoa.windows.open(name)` | `native_state.rs:771-813` | §5 body:320 | sup (3) | REAL | Queues the native window open; `invalid_argument` for other names. |
| `eqoa.mastery.get()` | `native_r6a.rs:143-152` | §7 | sup (4) | REAL | Mastery record from the core view. |
| `eqoa.mastery.skills()` | `native_r6a.rs:153-183` | §7 | sup (4) | REAL | Skill list; `unavailable` until sub-op 8. |
| `eqoa.mastery.selected()` | `native_r6a.rs:184-206` | §7 | sup (4) | REAL | Inspected node or nil. |

### 2.11 `eqoa.map`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.map.get()` | `native_state.rs:132-148` | §7 | sup (4) | REAL | Native map state plus `search_anchor`. |
| `eqoa.map.open()` | `native_state.rs:109-130` | §7 | sup (4) | REAL | Queues the M-map open; `unavailable` when Map is off, `busy` when the queue is full. |
| `eqoa.map.markers(world_id)` | `native_state.rs:71-107` | §7 | sup (4) | REAL | Static native markers for the world plus the session's native dynamic markers, `invalid_argument` outside 0..255. |
| `eqoa.map.view{size,layers}` | `map_views.rs:183-350` | §7 | sup (4) | REAL | Held native minimap capture with real `project/unproject/texture`; quota 4. |
| `eqoa.map.set_guide{...}` | `guides.rs:64-173` | §7 | sup (4) | REAL | Stores the guide, validates the entity/world, emits `guide.lost` when lost (`guides.rs:58-61`). |
| `eqoa.map.clear_guide()` | `guides.rs:175-183` | §7 | sup (4) | REAL | Removes this VM's guide and returns true. |
| `eqoa.map.images(world_id)` | **not registered** | §7:573, index /4 | ny (4) | DIAGNOSTIC | Seed stub only (`native.rs:107`); no binding anywhere (`map_views.rs:181-183` binds only `view`). |
| `eqoa.map.load_image(id)` | **not registered** | §7:574, index /4 | ny (4) | DIAGNOSTIC | Same: `nil,not_yet`. |
| `eqoa.map.set_waypoint(p)` | **not registered** | §7:575, index /4 | ny (4) | DIAGNOSTIC | Same: `nil,not_yet`, so the personal-waypoint service is unreachable from Lua. |
| `eqoa.map.clear_waypoint()` | **not registered** | §7:576, index /4 | ny (4) | DIAGNOSTIC | Same. |

### 2.12 `eqoa.mods`

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `eqoa.mods.send(mod,cmd,args?)` | `exchange.rs:159-230` | §Open Q:905 | sup (4) | REAL | Validates the target's declared commands, bounds the payload, queues `mod.command` for the next boundary; `unavailable` for missing/disabled targets. |

### 2.13 `eqoa.actions` (49)

All queue-time validation for the *queueing* actions is shared: `enqueue_lua_with`
(`action_broker.rs:2514-2536`) requires a live gesture token (`gesture::peek_token`), builds the
`ActionKind`, enqueues and returns a request handle; without a gesture they return
`nil,needs_player_action` with a diagnostic. The three queries (`status`, `cancel`,
`batch_status`) do not queue and are exempt. Dispatch is in `fork_ext/play.rs:960-2464`.

| Symbol | Registered at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `status(id)` | `action_broker.rs:3126` | §8 | sup (4) | REAL | Returns the retained `ActionResult`; `not_found` outside the owner's 256. |
| `cancel(id)` | `action_broker.rs:3197` | §8 | sup (4) | REAL | Cancels only an unsent request. |
| `batch(spec)` | `action_broker.rs:3262-3294` | §8 Part C | sup (4) | REAL | Parses 1..64 steps, requires the gesture, `enqueue_batch` (`action_broker.rs:692-770`). |
| `stop_batch(id?)` | `action_broker.rs:3295-3337` | §8 Part C | sup (4) | REAL | Stops the running batch (`action_broker.rs:786`). |
| `batch_status(id)` | `action_broker.rs:3338-3383` | §8 Part C | sup (4) | REAL | Retained `BatchResult`; owner check. |
| `target(ref?)` | `action_broker.rs:3743` | §8 | sup (4) | REAL | Dispatches `target_id`/clear (`play.rs:979`). |
| `cycle_target(group,filter)` | `action_broker.rs:3754` | §8 | sup (4) | REAL | Validates the filter, dispatches the native cycle (`play.rs:1009`). |
| `assist()` | `action_broker.rs:3385-3395` | §8 | sup (3) | REAL | `ActionKind::Assist` (`play.rs:960`). |
| `target_target()` | `action_broker.rs:3385-3395` | §8 | sup (3) | REAL | Same native assist path, as the reference action. |
| `toggle_stats_box()` | `native_state.rs:1193-1215` | §8 | sup (3) | REAL | Addon-locked, persists the toggle, says the native line; `permission_denied` for other addons. |
| `interact()` | `action_broker.rs:3776` | §8 | sup (4) | REAL | `interact_with_target` path (`play.rs:1054`). |
| `attack(down?)` | `action_broker.rs:3785` | §8 | sup (4) | REAL | Native press/release (`play.rs:1077`). |
| `cast(ability,opts)` | `action_broker.rs:3794` | §8 | sup (4) | REAL | Slot/lockout validation, native `send_0032` (`play.rs:1120`). |
| `use_hotbar(bar,slot)` | `action_broker.rs:3805` | §8 | sup (4) | REAL | Resolves the live menu entry (`play.rs:1197`). |
| `use_item(item)` | `action_broker.rs:3464` | §8 | sup (4) | REAL | Inventory-only resolve + native use (`play.rs:1581`). |
| `equip(item,equipped?)` | `action_broker.rs:3482` | §8 | sup (4) | REAL | ItemOut::UseItem path, idempotent when already in the wanted state (`play.rs:1617`). |
| `destroy(item,qty)` | **described only** | §8:623 | unsup | DIAGNOSTIC | `describe(...,"unsupported","not offered before the PR")` (`action_broker.rs:3110-3116`); the seed stub returns that error. Deliberate per plan 2026-11-16. |
| `swap_items(a,b)` | `action_broker.rs:3500` | §8 | sup (4) | REAL | Distinct inventory stacks, ItemOut::Swap (`play.rs:1659`). |
| `quick_slot(item,slot)` | `action_broker.rs:3525` | §8 | sup (4) | REAL | Native eligibility, ItemOut::QuickSlot (`play.rs:1709`). |
| `weapon_set(index)` | `action_broker.rs:3954` | §8 | sup (4) | REAL | Feature/entry checks (`play.rs:1420`). |
| `prepare_weapon_set(index,name)` | `action_broker.rs:3967` | §8 | sup (4) | REAL | ItemOut::GearSet with native length checks (`play.rs:1441`). |
| `assign_ability(slot,ability)` | `action_broker.rs:3997` | §8 | sup (4) | REAL | Send0033 path (`play.rs:1469`). |
| `unmemorize_ability(ref)` | `action_broker.rs:4014` | §8 | sup (4) | REAL | Native toggle by slot (`play.rs:1506`). |
| `unmemorize_all()` | `action_broker.rs:4040` | §8 | sup (4) | REAL | Send00FE (`play.rs:1537`). |
| `arrange_ability(a,b)` | `action_broker.rs:4049` | §8 | sup (4) | REAL | Send003A (`play.rs:1550`). |
| `loot(item,qty)` | `action_broker.rs:3549` | §8 | sup (4) | REAL | LootOut::One with native pending/capacity checks (`play.rs:1754`). |
| `loot_all()` | `action_broker.rs:3572` | §8 | sup (4) | REAL | LootOut::All (`play.rs:1819`). |
| `buy(item,qty)` | `action_broker.rs:3579` | §8 | sup (4) | REAL | MerchantOut::Buy with the live ware position (`play.rs:1858`). |
| `sell(item,qty)` | `action_broker.rs:3601` | §8 | sup (4) | REAL | MerchantOut::Sell with the native quote (`play.rs:1940`). |
| `bank_move(item,dest,qty)` | `action_broker.rs:3623` | §8 | sup (4) | REAL | BankOut::Move, banker state gate (`play.rs:2000`). |
| `bank_money(dir,amount)` | `action_broker.rs:3700` | §8 | sup (4) | REAL | BankOut::MoveMoney (`play.rs:2079`). |
| `chat(text)` | `action_broker.rs:3840` | §8 | sup (4) | REAL | Report guard + length/NUL checks in `validate_chat_string`, then `chat_line` (`play.rs:1236`). |
| `command(text)` | `action_broker.rs:3853` | §8 | sup (4) | REAL | Same guard; a missing `/` is normalised, never silently sent as chat. |
| `move(...)` | **described only** | §8:638 | ny (4) | DIAGNOSTIC | `describe(...,"not_yet","continuous controls come after the PR")` (`action_broker.rs:3117-3125`). |
| `look(...)` | **described only** | §8:639 | ny (4) | DIAGNOSTIC | Same. |
| `autorun(on?)` | `action_broker.rs:3866` | §8 | sup (4) | REAL | Conditional native toggle (`play.rs:1246`). |
| `follow(ref?)` | `action_broker.rs:3875` | §8 | sup (4) | REAL | Same-scene check, native toggle, verifies `following` (`play.rs:1262`). |
| `stop()` | `action_broker.rs:3888` | §8 | sup (4) | REAL | Releases attack/autorun/follow and this VM's leases (`play.rs:1315`). |
| `camera(opts)` | `action_broker.rs:3397` | §8 | sup (4) | REAL | Field validation then native camera setters (`play.rs:2119`). |
| `open_window(kind,arg?)` | `action_broker.rs:3895` | §8 | sup (4) | REAL | Per-kind native open; `invalid_argument` for unknown kinds, `not_ready` when assets/guild/mode block it (`play.rs:1328-1419`). |
| `close_window(ref)` | `window_actions.rs:205` | §8 | sup (4) | REAL | Live window + focus + signature revalidation, then key 132 (`play.rs:2327`). |
| `select_row(ref,row)` | `window_actions.rs:217` | §8 | sup (4) | REAL | Row bounds, enabled/report guard, walks the native selection (`play.rs:2334`). |
| `activate_row(ref,row)` | `window_actions.rs:217` | §8 | sup (4) | REAL | Adds the confirm key after the same validation (`play.rs:2363`). |
| `dialog_answer(ref,opt)` | `window_actions.rs:241` | §8 | sup (4) | REAL | Per-mode option count, confirm selection handling (`play.rs:2397`). |
| `input_text(ref,text,submit?)` | `window_actions.rs:272` | §8 | sup (4) | REAL | Password/report guards, paste round-trip check, optional `edit_done` (`play.rs:2432`). |
| `drop_quest(ref)` | `window_actions.rs:136` | §8 | sup (4) | REAL | Revision+stamp+title check then `Quests::send_remove` (`play.rs:2252`). |
| `mastery_share(frac)` | `window_actions.rs:166` | §8 | sup (4) | REAL | 0.01-step check, `Part::select` (`play.rs:2266`). |
| `mastery_browse/buy/refund(id)` | `window_actions.rs:188-204` | §8 | sup (4) | REAL | Window/pending/list checks, inspected-node match for buy/refund (`play.rs:2279-2325`). |
| `buttons(...)` | **described only** | §8:674 | ny (4) | DIAGNOSTIC | `describe(...,"not_yet")` (`action_broker.rs:3117-3125`). |
| `menu_command(ref,cmd,arg?)` | `window_actions.rs:297` | §8 | sup (4) | REAL | Command must be an enabled row, non-zero args refused, then native confirm (`play.rs:2372`). |

### 2.14 `frontiers_forge.*`

`annotate_host` (`native.rs:293-309`) declares every module `partial` and every member
`supported`, so the `cap` column below is the *reported* status; the verdict is what the member
actually does. The six deliberately unsupported members are re-described afterwards
(`native.rs:310-325`) and the two item/merchant window reads that were deferred are no longer
deferred (`items_forge.rs:16`, `DEFERRED` is empty).

| Symbol group | Built at | In spec? | cap | Verdict | Evidence |
|---|---|---|---|---|---|
| `frontiers_forge.ability.*` (Get/Set/Cast/cooldown/live methods, ~40) | `ability.rs:390-404` | §11 | sup (2) | REAL | Every method re-resolves the native ability ref on call; `Ability.new` only accepts native tokens. |
| `frontiers_forge.ability_list.*` (index/display-index, lockout subscriptions) | `ability.rs:408-882` | §11 | sup (2) | REAL | Iteration and subscriptions over the native list + event bus. |
| `frontiers_forge.ability_bar.*` (3 bars × 5 slots, item/ability slots) | `ability.rs:886-1160` | §11 | sup (2) | REAL | 3 bars (`MAX_BARS`), out-of-range bar/slot raises the pinned invalid-index error (`ability.rs:921-925`), `GetSlotUITexId` keeps the pinned glyph table. |
| `frontiers_forge.bank.SlotsUsed/GetItems` | `items_forge.rs:398-419` | §11 | sup (2) | REAL | Reads `eqoa.bank.get()`; contents survive a closed window. |
| `frontiers_forge.camera.GetCoordinates` | `host.rs:245-261` | §11 | sup (4) | PARTIAL | Returns `{0,0,0}` when no camera state exists (`host.rs:250-254`) with no diagnostic — a zero substitute where the eqoa API says `unavailable`. |
| `frontiers_forge.camera.GetFacingRadians` | `host.rs:262-268` | §11 | sup (4) | REAL | Orbit facing, nil when unknown. |
| `frontiers_forge.camera.GetFacingDegrees` | `host.rs:269-277` | §11 | sup (4) | REAL | Same value in degrees, 0.0 when unknown (legacy shape). |
| `frontiers_forge.cave.*` (7 functions) | **no module table** | §11:738 | unsup (module/Alloc) | DIAGNOSTIC (partial) | Only `frontiers_forge.cave` and `.Alloc` exist as descriptors (`native.rs:155-171`); the package loader answers `require("frontiers_forge.cave")` with "unsupported FrontiersForge API" (`package.rs:713-723` shows the same behavior for `quest`), so a high-level module that requires it cannot load, contrary to §11. |
| `frontiers_forge.chat.*` (message types, send, hooks, history) | `chat.rs:140-525` | §11 | sup (2) | REAL | Real queue (`OUTGOING_CHAT`), real pending count, report guard on send. |
| `frontiers_forge.chat.IsSendHookSuspended` | `chat.rs:477-480` | §11 | sup (2) | PARTIAL | Hard-coded `false` with no suspension mechanism behind it and no diagnostic. |
| `frontiers_forge.chat.CaptureChannelTypeFromGameWindow` | `chat.rs:507-518` | §11:770 | unsup | DIAGNOSTIC | Returns `nil,reason,nil` and diagnoses. |
| `frontiers_forge.chat.GetChannelTableInfo` / `DebugDumpFocusChain` | `chat.rs:520+` | §11:771 | unsup | DIAGNOSTIC | Same explicit refusal, no fabricated tables. |
| `frontiers_forge.combat.*` (four Dealt/Received handlers, flags) | `combat.rs:150-468` | §11 | sup (2) | REAL | Event-driven, RGB tagging in the adapter only. |
| `frontiers_forge.combat.ReadForeignCave` | `combat.rs:276-284` | §11:740 | unsup | DIAGNOSTIC | Explicit "no foreign code cave". |
| `frontiers_forge.effects.*` (max_effects, count, per-index, All) | `effects.rs:100-164` | §11 | sup (2) | REAL | 0..7 index base, drinks from the native effect list. |
| `frontiers_forge.entity.*` (record methods/properties, target id) | `entity.rs:110-729` | §11 | sup (2) | REAL | Live re-resolution per call; `GetTargetId` narrows to `& 0xff` per the pinned reference. |
| `frontiers_forge.entity_list.*` (0..23 slots, GetAllEntities) | `entity.rs:745-858` | §11 | sup (2) | REAL | Native slots preserved, empty slots returned as the reference does. |
| `frontiers_forge.gems.IsGem/Get/Format` | `items_forge.rs:556-577` | §11 | sup (2) | REAL | Calls the real `eqoa.gems.get`, fixing the reference's lowercase typo. |
| `frontiers_forge.group.*` (count, members, HP/HP255, coordinates) | `group.rs:60-264` | §11 | sup (2) | REAL | 0-based member indexes, nil HP when unknown, sentinels only where the reference uses them. |
| `frontiers_forge.icon.Acquire/Release/GetOwnerCount/HasClaim` | `icons.rs:1121-1155` | §11 | sup (2) | REAL | Real per-VM owner set behind the cache claims. |
| `frontiers_forge.icon.GetTexture` | `icons.rs:1157-1162` | §11:746 | sup (2) | FAKE SUCCESS | Returns `(nil,nil,nil)` for every input with no diagnostic, while `eqoa.icons.resource` loads the same art. The replacement exists in `attach_icon_cache` (`icons.rs:1215+`), but its only caller is the `#[cfg(test)] fn install_test_gpu_for_host` (`icons.rs:1637-1661`), so production keeps this stub. |
| `frontiers_forge.icon.GetUITexture` | `icons.rs:1164-1172` | §11:746 | sup (2) | PARTIAL | Returns `(nil,nil,nil)`; prints one `eprintln!` the first time and nothing afterwards, never a manager diagnostic. |
| `frontiers_forge.icon.GetSurfaceInfo` | `icons.rs:1174-1197` | §11:746 | sup (2) | REAL | Real width/height from the live library; `format` is the pinned RGBA32 constant. |
| `frontiers_forge.input.*` (masks, PollKeyEvents, capture, hooks) | `input.rs:200-955` | §11 | sup (2) | REAL | Real report access and per-owner cursors; install/hooks gate the native delivery lease. |
| `frontiers_forge.inventory.*` (14 members + list object) | `items_forge.rs:207-395` | §11 | sup (2) | REAL | Dense 1-based slots, tab enum 0..3 with game labels, list rows from the native page state. |
| `frontiers_forge.item.*` (34 getters + methods/properties) | `items_forge.rs:18-204` | §11 | sup (2) | REAL | Maps to `eqoa.items.get`; `IsValid` compares name+slot as the reference does. |
| `frontiers_forge.loot.*` (7) | `items_forge.rs:421-464` | §11 | sup (2) | REAL | 1-based, 40 cap, `{}`/0 when closed. |
| `frontiers_forge.merchant.*` (13 + button hint) | `items_forge.rs:467-553` | §11 | sup (2) | REAL | Mode/option/selection from the approved `Merchant::read_state`; hint through `eqoa.windows.format_hint`. |
| `frontiers_forge.player.*` (~68 Get*/properties) | `player.rs:200-787`; `health.rs:228-240` (skipped when player.rs beat it, `visibility.rs:251-253`) | §11 | sup (2) | REAL | Flat getters over the native snapshot; CM getters return nil until the class-mastery counters exist (`player.rs:488-526`). |
| `frontiers_forge.quest` (`new`, `GetName`) | **not registered** | §11:753 | unk | DIAGNOSTIC (absent) | `package.rs:716-721` asserts `require('frontiers_forge.quest')` fails with "unsupported FrontiersForge API"; not described in capabilities either. |
| `frontiers_forge.quest_log.*` (5 + quest objects) | `quest_log.rs:60-101` | §11 | sup (2) | REAL | 8-slot, 0-based, title-only objects re-resolved on revision. |
| `frontiers_forge.ui.*` (Disable*/Enable* pairs, icons, hidden-by) | `visibility.rs:255-603`; `player.rs`/`health.rs` health pair | §11:755 | sup (2) | REAL | Owner-count pairs over the same visibility intent as `eqoa.hud`; class/disposition textures real. |
| `frontiers_forge.util.*` (IsInGame, IsStartMenuOpen, compass, bearing, XP table, bit test, Log) | `host.rs:140-363` | §11:756 | unk (no descriptors) | REAL (with gaps) | Values are real, but `capabilities.list()` never shows them (no host module, `host.rs:546`), `GetWorldId` is absent, and `utf8_to_utf16`/`utf16_to_utf8` diagnose unsupported (`host.rs:336-338`). |
| `frontiers_forge.window.*` (Mode/ConfirmStyle/InputTag constants, hints, IsTextDialogOpen) | `host.rs:180-241` | §11:757 | sup (2/4) | REAL | Constants and hint formatting are pure; dialog state reads the live mode. |
| `frontiers_forge.window.WindowObject` memory methods (`Read`, `ReadText`, `GetDrawAddress`, …) | `items_forge.rs:320-333` | §11:757 | (object method, not a capability path) | DIAGNOSTIC | `memory(name)` diagnoses `unsupported` and returns nil; no emulator address is invented. |
| `UiForge.*` globals (Log, CallbackType, RegisterCallback, paths) | `host.rs:340-460`; `native.rs:355-359` | §11:758 | sup (2) | REAL | Real callback registry (`CallbackType` 0..4, `host.rs:364-390`), bounded log queue, per-addon `scripts_path`/`resources_path`. |
| `UiForge.IGraphicsApi.CreateTextureFromFile` | `textures.rs:462-500` | §11:772 | sup (2) (as `UiForge.IGraphicsApi`) | REAL | Uses the same texture service and returns `texture,w,h`. |
| `ffi` | `native.rs:158-171` | §13 | unsup | DIAGNOSTIC | Described unsupported, "raw memory and FFI are excluded"; no module table. |

## 3. Mismatches

### 3.1 In the spec, not registered (or registered as a stub that never landed)

| Spec | Spec line | What the code does | Effect |
|---|---|---|---|
| `eqoa.map.images` | §7:573, index:96 | Not bound anywhere; seed stub `not_yet` (`native.rs:107`). | Round-4 function still missing at round 7. |
| `eqoa.map.load_image` | §7:574 | Same. | Map image API unreachable from Lua. |
| `eqoa.map.set_waypoint` | §7:575 | Same. | The personal-waypoint service is unreachable from Lua; only mod-owned guides exist. |
| `eqoa.map.clear_waypoint` | §7:576 | Same. | Same. |
| `eqoa.files.list` | §5:420, index:83 | Not bound; `file_io.rs:538` registers read/write/remove only. | Mods cannot enumerate their private area. |
| `eqoa.fonts.game` | index:84 only (no §5 body) | Not bound, not described. | Calling it is a raw Lua error, not a diagnostic. |
| `frontiers_forge.quest` | §11:753 | Deliberately unsupported (`package.rs:716-721`), and not in capabilities as `unsupported` either (`status="unknown"`). | Forge mods using `Quest.new` cannot even feature-detect it. |
| `frontiers_forge.cave` (7 functions) | §11:738 | Only 2 descriptors; `require` throws. | A high-level module that requires cave crashes instead of seeing per-call diagnostics. |
| `frontiers_forge.util.GetWorldId` | §11:756 | Absent from the module. | Mods get a nil-call error; no diagnostic or capability row. |
| `eqoa.actions.*` without a gesture | §8:594 says all actions work from any callback | Every queueing action requires a gesture (`action_broker.rs:2514-2524`); GETTING-STARTED:164 already documents the gate. | The spec body contradicts both the code and the owner decision (plan 2026-11-16). |
| `eqoa.actions.destroy` | §8:623 | `unsupported` diagnostic (`action_broker.rs:3110-3116`). | Intentional per the plan, but the spec still promises it. |
| `eqoa.actions.move/look/buttons` | §8:638-639, 674 | `not_yet` diagnostics (`action_broker.rs:3117-3125`). | Same: plan says after the PR; spec says round 4. |

### 3.2 Registered but missing from the spec's normative index

The index at §2:64-111 claims to be "the normative capability spelling index" and instructs
implementers to compare their descriptor sets against it. It has not been updated since round 1,
so it disagrees with the shipped surface in both directions:

- `eqoa.actions.batch`, `.stop_batch`, `.batch_status` (spec'd in §8 Part C but absent from the index row for `actions`).
- `eqoa.mods.send` (index has no `mods` module).
- `eqoa.entities.dots`, `.pick_dot` (documented in §4 body:227-228, not in the index).
- `eqoa.map.markers`, `.open`, `.view`, `.set_guide`, `.clear_guide` (documented in §7:577-580, not in the index).
- `eqoa.files.load_json`, `.save_json_async`, `.import`, `.export`, `.null` (Open questions section, not in the index).
- `eqoa.options.button`, `eqoa.input.pointer`, `.claim_pointer`, `.minimap_wheel`, `eqoa.windows.open`.
- `eqoa.icons.class`, `.disposition` are in the index (marked /4) but `eqoa.icons.SurfaceInfo.source_format` is declared supported while always nil.

Conversely, `eqoa.client.time` is checked for argument arity at `native.rs:735` but is never
registered or described — a dead symbol reference.

### 3.3 Capabilities disagreeing with the code

1. **Every Forge member is `supported`** (`native.rs:302-307`), including
   `frontiers_forge.icon.GetTexture`, which always returns `(nil,nil,nil)`. Six members are
   corrected by hand afterwards (`native.rs:310-325`); the rest are unverified claims.
2. **`eqoa.items.MerchantState.button_hint` is `not_yet` at round 6** (`native_items.rs:597-604`,
   reason "needs windows.format_hint, which is not built yet") while `eqoa.windows.format_hint`
   is registered (`native_r6a.rs:330`) and `frontiers_forge.merchant.GetButtonHint` uses it
   (`items_forge.rs:548-552`). The field is populated; the descriptor is stale.
3. **`eqoa.icons.SurfaceInfo.source_format`** is `supported` (`native.rs:1231`) but hard-coded
   `null` at `icons.rs:1516`.
4. **`bit.*` has no capability rows at all** (§2:146 says capabilities use `bit.<name>`), so
   `capabilities.get("bit.band")` answers `unknown` and `list("bit")` is empty.
5. **`frontiers_forge.util`** is reachable by `require` but invisible to `capabilities.list()`
   because it is not a registered host module.
6. **`hud.hidden_by` placeholder** (`native.rs:958`) returns an empty table silently; it is only
   replaced when a visibility lease is installed (`visibility.rs:238-253`). Any host that does
   not install one (`Host::new`, used by `gpu_test.rs` and `report_viewer.rs`) exposes a silent
   `{}` instead of a diagnostic.
7. **Module status is recomputed only on `bind`** (`native.rs:814-837`): a module created purely
   by `describe` (e.g. `eqoa.imgui`, corrected to `partial` by hand at `native_media.rs:25-31`)
   can end up with a status that does not reflect its children.

### 3.4 Round labels

The plan's rounds are 2, 2.5, 3, 4, 5 (items), 6 (actions), 7 (finish). The capability rounds
are a mix of the spec's old numbering and the plan's:

| Descriptor | Reported round | Plan round | Note |
|---|---|---|---|
| `eqoa.items.get` and every other `items` function | 3 (`native.rs:99-102`) | 5 | Seed kept the spec's "round 3". |
| `eqoa.items.Item` and its fields | 5 (`native_items.rs:592-594`) | 5 | The same module's *records* were renumbered, its *functions* were not. |
| `eqoa.actions.batch/stop_batch/batch_status` | 4 (`native.rs:70`) | 6 | Actions are "the rest" in plan round 6. |
| `eqoa.quests.*`, `eqoa.camera.*`, `eqoa.windows.*`, `eqoa.mastery.*` | 4 (`native_r6a.rs:422`, seed) | 6 | Built in round 6, labelled 4. |
| `eqoa.map.*` | 4 (`native.rs:107`) | 4 | Consistent by accident (map is plan round 4). |
| `eqoa.hud.*`, `eqoa.options.*` | 3 (seed + `native.rs:1248-1255`) | 3 | Consistent. |
| `eqoa.files.load_json/…` | 4 | 4 | Consistent. |

The spec text is equally mixed: §6 is titled "round 3", §7/§8 "round 4", §12's gate table says
"Round 5 Finish" (the plan's round 5 is items). Nothing in the docs was renumbered for round 7,
which is a named deliverable of round 7 in `docs/plans/EQOA-MOD-API-PLAN.md:340-341`.

## 4. Fix list (by severity)

1. **`frontiers_forge.icon.GetTexture` is a silent nil** (`icons.rs:1157-1162`). Smallest fix:
   route it through `with_active_provider` + the icon cache (the code already exists in
   `attach_icon_cache`, `icons.rs:1215+`) or call `error(lua,"frontiers_forge.icon.GetTexture",
   "not_yet"/"unavailable", …)` and describe it accordingly. Same for `GetUITexture`
   (`icons.rs:1164-1172`), which currently only `eprintln!`s once.
2. **`hud.hidden_by` falls back to a silent `{}`** (`native.rs:958-963`). Fix: make the
   placeholder return
   `error(lua,"eqoa.hud.hidden_by","unsupported","no visibility lease on this host")` so a host
   without `visibility::install_native` is visible instead of plausible.
3. **`eqoa.map.images/load_image/set_waypoint/clear_waypoint` and `eqoa.files.list` are still
   seeds** (`native.rs:85,107`; `map_views.rs:181`, `file_io.rs:538`). Fix: either implement them
   or move their descriptors to `unsupported` with the owner's reason so a round-7 reader is not
   told "not_yet, implementation pending" for round-4 work; the smallest honest change is the
   descriptor text, recorded in the support table round 7 is meant to generate
   (`docs/plans/EQOA-MOD-API-PLAN.md:339-340`; neither `docs/modding/SUPPORT-TABLE.md` nor
   `docs/modding/AGENT-GUIDE.md` exists yet).
4. **Forge modules can disappear or lie in capabilities**: `annotate_host` marks every member
   `supported` (`native.rs:302-307`) and `frontiers_forge.quest`/`.cave`/`.util` are not
   describable. Fix: describe from an explicit per-module allowlist (or mark a member
   `supported` only when it is not one of the known stubs), add `frontiers_forge.quest` as
   `unsupported`, add `frontiers_forge.cave` and its seven functions as `unsupported` with a
   real guarded module table, and register `frontiers_forge.util` as a host module.
5. **Stale descriptor for `eqoa.items.MerchantState.button_hint`** (`native_items.rs:597-604`).
   Fix: delete the `not_yet` describe; `install_fields` already declares the field.
6. **Round labels** (`native.rs:65-144`, `native_r6a.rs:422`, `native_items.rs:592`). Fix:
   renumber the seed to the plan (items 5, actions 6, quests/camera/windows/mastery 6) and
   update the spec's section headers; this is the round-7 deliverable at
   `docs/plans/EQOA-MOD-API-PLAN.md:340-341`.
7. **The spec's §8 preamble promises gesture-free automation** (§8:594) while the code returns
   `needs_player_action` (`action_broker.rs:2514-2524`) and GETTING-STARTED:164 already documents
   the gate. Fix: replace §8:594 with the owner's 2026-11-16 rule.
8. **`eqoa.icons.info().source_format`** is declared supported but always nil
   (`icons.rs:1514-1518`, `native.rs:1231`). Fix: describe the field `not_yet`/`unsupported`
   (the spec permits nil + per-field diagnostic) or populate it from the decoder when it knows.
9. **`bit.*` has no capabilities** (§2:146). Fix: `describe` the eight names as `supported` and
   make the module's `__index` diagnose unknown members (right now `bit.foo` is plain nil).
10. **`frontiers_forge.chat.IsSendHookSuspended` returns a constant** (`chat.rs:477-480`). Fix:
    either implement the suspension or describe it `unsupported`. `frontiers_forge.camera.
    GetCoordinates` returning `{0,0,0}` (`host.rs:250-254`) should return nil or diagnose until
    the camera state exists.
11. **`eqoa.client.time`** is referenced in the arity guard (`native.rs:735`) but never
    registered; remove the string or add the symbol to the seed so the guard is not misleading.
12. **Documentation drift**: `items_forge.rs:9-11` still says the button hint "says it is not
    built (DEFERRED)" and that Forge transactions "are not built in this round", while
    `DEFERRED` is empty (`items_forge.rs:16`). Fix the module comment.

### Not audited / UNCLEAR

- **`eqoa.imgui` bridge members**: the registration and the `not_yet`/`unknown_symbol` behavior
  are real (`native_media.rs:8-87`), but whether each of the ≈110 bindings matches its Dear ImGui
  semantics was not verified symbol by symbol (the bridge is covered by its own test suite).
  Verdict for the namespace as a whole: REAL for registration, UNCLEAR for per-member parity.
- **`frontiers_forge.input` poll/capture semantics** (owner cursors, suppression) were read at
  the registration level only; the per-member `PollKeyEvents` cursor behavior was not exercised.
- **`frontiers_forge.player` CM getters**: `health.rs:228-240` would register a two-function
  `player` module if it ever ran first, but `visibility.rs:251-253` skips it whenever
  `player.rs` already registered the module, so the full table wins. If the install order is
  ever reversed, `frontiers_forge.player` silently degrades to two functions; that ordering is
  worth a test rather than a doc note.
