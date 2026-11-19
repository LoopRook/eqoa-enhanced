# UI coverage audit: what mods still need to replace or extend the whole game UI

Planning audit, written 2026-11-19. No game code was changed and nothing was compiled or run. It answers one
question for the owner: what is still missing before mods can fully replace or extend the game's UI (ElvUI style)
and build helper mods, feature by feature, and in what order to build it after round 7.

It extends [window-replacement-design.md](window-replacement-design.md), which covers the windows. This audit
covers every player-facing feature, window or not.

**Baseline.** Branch `docs/round-7-audit` at `e850996a`: develop with round 6.5 and round 7 Parts A and B, merged
with `sync/2026-11-18` (upstream through `35e2a43b`). Paths are relative to `crates/eqoa/src/`; `net/` means
`crates/net/src/`. The window design's line numbers are older than this sync: `hud.rs`, `hudlayout.rs` and
`raidplay.rs` did not move, `play.rs` moved by about 590 lines. This document gives the current numbers.

**Method.** Five read-only surveys of the game code (one per feature group) plus the lead's own read of the fork
side: `docs/modding/SUPPORT-TABLE.md` (generated 2026-11-19), `fork_ext/play.rs`, `fork_ext/r6a_capture.rs`,
`fork_ext/addons/window_takeover.rs`, the plan, the backlog and the round 6.5 and 7 reports. Every claim that
decides a seam count, and every defect listed below, was read again by hand. "Not verified" at the end lists what
was not.

## Summary

- **Where mods stand today.** Bags, bank, merchant and corpse are done end to end (reads, actions, takeover).
  Abilities, hotbars, quests, class mastery, the group list, chat and most of the HUD have their reads and many
  actions. Everything else can at most be opened or named: of the game's 44 window modes, a mod can read the
  contents of 12 (`fork_ext/r6a_capture.rs:443-676`).
- **The largest gap is not windows.** It is the contract of round W1 (session, pass, focus, `replaces`), which
  nothing but corpse, merchant and bank can use yet, and a set of reads and small actions the client already
  holds data for: buff time left, cast times, pet commands, group commands, quest objectives, training points.
  Those need **no upstream line**.
- **Upstream lines.** The design's 10 firm inserted lines stand. This audit adds 8 probable lines (one per window
  whose replies or state are private to its file) and 9 optional ones. **Firm 10, probable 18, ceiling 27.** The
  design said 10 and 22; the difference is the mercenary windows, the dye picker, crafting's reply check and the
  world overlays (name plates, damage numbers), which the design did not cover.
- **Four defects in a supported `eqoa/1` action**, to fix in round 7 before the freeze (section 2).
- **One policy gap**: upstream's new "game as it was" servers and [OG] accounts switch the client's own extras
  off. Mods are not covered by that switch (section 3, owner question 1).
- **Scale after round 7:** 14 small rounds. The first five carry most of the value and cost 10 upstream lines.

## 1. What changed since the window design

| Change | Effect on the plan |
|---|---|
| Round 6.5 is merged (`91a66d7d`), owner tests passed 2026-11-19 | The takeover registry, scopes, hidden sessions, capability gate and label exist. W0 can start. |
| Round 7 Parts A and B are merged; C has not started (`feat/eqoa-api-r7c` equals the sync tip) | The three contract rules the owner asked to settle before the freeze are **not built**: `takeover` still answers `invalid_argument` for every kind but loot, merchant and bank (`fork_ext/addons/window_takeover.rs:55-64`), and `replaces` does not exist. |
| The HUD has 12 movable pieces, not 11: `SpellBars` was added (`hudlayout.rs:35-66`) | Seam 3 still covers all of them with one line. |
| New window modes 61, 62 (mercenary inventory and spells, `mercplay.rs:30-31`) and 63 (dye picker, `dyeplay.rs:17`) | The fork's name table stops at 60 (`fork_ext/r6a_capture.rs:89-134`); these read as "other". Two more files for the long tail. |
| Hardcore characters, memorials on character select (`pregame/client/select.rs:44-88`, `play.rs:14158`) | No mod surface. Section 5.30. |
| Server restrictions `settings::qol` (`settings.rs:2254-2330`, set each frame at `play.rs:3594`) | Mods ignore them. Section 3. |
| The buff window (`buffwnd.rs`, `net/buffs.rs:36-49`) | The client now holds time left and "removable" per buff. `eqoa.effects` exposes neither. |
| Quest objectives from the server (`net/session.rs:936-950`, op 0x7E06) | The design said the client holds no objective progress. On servers that send 0x7E06 it does. |

## 2. Defects found in `eqoa/1` as built (fix in round 7, before the freeze)

All four are in `eqoa.actions.open_window`, which the support table lists as supported. Changing what a supported
name does after the freeze is a breaking change, so these belong to round 7 Part C or G.

| # | Call | What it does today | Evidence | Fix |
|---|---|---|---|---|
| D1 | `open_window("auction")`, `("auction_browse")` | Opens the **Tell** form (mode 28) | `fork_ext/play.rs:2029` sends `0x0B0D_0000`; `play.rs:7793` maps that to `open_mail(28)` | Use `0xA801_0000` through `do_action` (`play.rs:8540`); refuse `not_ready` when the bank does not offer the auction |
| D2 | `open_window("auction_search")` | Opens the **Reply** form (mode 29) | `fork_ext/play.rs:2030`; `play.rs:7794` | Same; add `"tell"` and `"reply"` as their own kinds |
| D3 | `open_window("mail")` and `("mail_compose")` | Swapped: "mail" opens compose (23), "mail_compose" opens the inbox (24) | `fork_ext/play.rs:2027-2028`; `play.rs:7791-7792`; `mail.rs:77-79` names 23 compose, 24 inbox | Swap the two ids |
| D4 | `open_window("who")` | Shows the who window without asking the server, so the list is old or empty until Refresh | `fork_ext/play.rs:2035-2039` sets mode 35 only; the game's own open sends `who.open()` first (`play.rs:8555-8561`) | Send the request as the game does |

Also for round 7 (additive, small): name modes 61, 62, 63 in `mode_name`; give the yes/no prompt read its **kind**
(group invite, summons, resurrection, pet dismiss, quit, guild, report). Today a summons and a resurrection both
read as tag 0 (`play.rs:7281-7293`, `fork_ext/r6a_capture.rs:603-625`), so a mod cannot tell them apart except by
the words.

One upstream defect seen in passing, not ours and not verified by a run: the client cuts a slash command's word to
10 characters, so its own `/weatherpreview` can never match (`play.rs:6412-6422`, `:6472`).

## 3. Gaps that cut across features

1. **The contract is not built.** `windows.session`, `pass`, `focus`, `replaces`, the "Replaces game windows"
   label and the "Use the game's windows" switch are all W1. Until then a replacement mod can only draw beside
   the game's window, never instead of it.
2. **Server replies that are dropped unless the native window is open.** Five places check the mode when a reply
   arrives. A replacement hides the window, so the reply is lost. Each needs one inserted line, or that window
   stays helper-only:
   - crafting: `play.rs:3865` builds `Modes { recipe: self.mode == 6, experiment: self.mode == 37 }`;
   - class mastery: the inspected skill is cleared unless `self.mode == 40` (`play.rs:7950`);
   - auction: results are cleared when the mode is not 39 (`auction.rs:354-357`);
   - dye picker: cancels itself when the mode leaves 63 (`dyeplay.rs:272-274`);
   - fishing: the window only runs in mode 36 (`fishplay.rs:209-213`).
3. **Actions private to a file.** `fork_ext/play.rs` is a child of `play.rs` (`play.rs:47-48`), so it calls
   `Play`'s private functions and the `pub(super)` functions of `play.rs`'s other children. It cannot call
   functions that are private inside those children: trade (`npcw.rs:324-372`), duels (`duelplay.rs:124`), the
   blacksmith's id (`support.rs:21-23`, `:199`), the auction search (`bankplay.rs:60`, `:266`), the dye picker
   (`dyeplay.rs:205`, `:219`), mercenary windows (`mercplay.rs:42-48`), ready checks and rolls
   (`raidplay.rs:213`, `:244`). One child-module line per file reaches them (seam 7 is the model).
4. **Hidden pieces that still take presses.** Beyond the click areas the design covers: a long press still takes
   a buff off and still toggles auto-cast with the buff window or the hotbar hidden (`play.rs:5896-5908`,
   `buffplay.rs:99`), and the buff window's keys are not gated (`buffplay.rs:23`, `:44`).
5. **Servers that ask for the game as it was.** The server names which of the client's additions are off
   (`settings::qol`: target level, DPS meter, power bars, buff window, auto-cast, quest helper, map; an [OG]
   account also loses the mouse). `fork_ext/` never reads those bits. The map APIs follow `settings::map()`
   (`fork_ext/play.rs:2003`), but a mod can still show a target's level, others' power, per-hit damage and its
   own timers there, and can act on its own. Whether mods obey the server's rule is the owner's call; it should
   be settled before the freeze because it changes what a supported read returns on those servers.
6. **Destructive actions.** `destroy` stays unsupported. The same question applies to guild disband and quit,
   pet dismiss, mail delete and buddy removal: each has a native yes/no today. Proposed rule: a typed action for
   an irreversible step opens the game's own prompt instead of skipping it (owner question 3).
7. **Looks.** `eqoa.fonts.game` is unsupported, so mod windows drawn with ImGui cannot use the game's own
   lettering; only the HUD layer (`eqoa.hud.text`) has it. A full replacement will look like the mod, not the game.
8. **Not started from round 7:** key conflicts between mods and game controls (Part E), and held controls
   (`move`, `look`, `buttons`), which stay after the upstream PR.

## 4. Coverage at a glance

"Today": **done** (reads, actions, takeover), **most** (reads and most actions), **part**, **open only**, **none**.
"Lines" are new inserted upstream lines beyond round 6.5's four; a number in brackets is optional.
Difficulty S/M/L is the size of one small round or less; risk is to the player. Rounds are those of section 7.

| # | Feature | Value | Today | Lines | Difficulty | Risk | Round |
|---:|---|---|---|---:|---|---|---|
| 1 | Bags, worn gear, item menu | high | most | 2 (W1's) | M | low | A1 |
| 2 | Corpse loot, merchant, bank | high | done | 0 | S | low | A0 |
| 3 | Spellbook and abilities | high | most | 0 | S | low | A1 |
| 4 | Hotbars, weapon sets, auto-cast | high | most | (1) | S | low | A3, A4 |
| 5 | Quest journal and tracker | high | part | 0 | S | low | A1, A3 |
| 6 | NPC dialogs (hail, quests, turn-ins) | highest | part | 0 | L | high | A2 |
| 7 | Yes/no prompts: invite, summons, resurrection | medium | part | 0 | M | medium | A2 |
| 8 | Player, target, pet, group frames | high | most | 4 (1) | M | low | A3, A4 |
| 9 | Buffs and effects | high | part | (1) | S | low | A3 |
| 10 | Cast bar | high | none | 0 | S | low | A3 |
| 11 | Pets | medium | part | 0 | S | low | A3 |
| 12 | Group: invite, leave, kick, disband | high | part | 0 | S | low | A3 |
| 13 | Raid: list, commands, ready check, rolls, master loot | medium | part | 4 | M | medium | A4 |
| 14 | Chat box, entry line, tabs, links | high | most | 0 | M | medium | A6 |
| 15 | Start menu | medium | part | 0 | M | medium | A6 |
| 16 | Character summary and stat training | medium | part | 0 | S | medium | A5 |
| 17 | Class masteries | medium | most | 1 | S | medium | A5 |
| 18 | Factions, combat record, DPS meter | low | open only | 0 | S | low | A5 |
| 19 | Friends and ignore | medium | open only | 0 | S | low | A7 |
| 20 | Guild | medium | open only | 0 | M | medium | A7 |
| 21 | Who, player search, group search | medium | open only | 0 | S | low | A7 |
| 22 | Crafting | medium | open only | 1 | M | medium | A8 |
| 23 | Trade between players, duels | medium | none | 2 | M | high | A9 |
| 24 | Blacksmith (repair) | medium | none | 1 | S | low | A9 |
| 25 | Mail and tells | low | open only | (1) | M | low | A10 |
| 26 | Auction | medium | none | 1 | L | high | A10 |
| 27 | Mercenaries, dye picker, fishing | low | none | 2 (1) | M | medium | A11 |
| 28 | Name plates, damage numbers, target rings | high | none | (3) | M | low | A12 |
| 29 | World map and minimap | low | most | (1) | M | low | A13 |
| 30 | Emotes, jump, death, experience, Hardcore | low | part | 0 | S | low | A3 |
| 31 | Options, keys, macros | low | part | 0 | - | - | keep native |
| 32 | Login, character select, creation | low | none | - | L | high | not proposed |

## 5. Feature by feature

### 5.1 Bags, worn gear, item menu (value: high)

- **Game:** not a mode of its own; it lives in the Start menu (mode 13). Items entry `0x0200_0000` opens it
  (`play.rs:7655-7659`). Requests are `ItemOut` (`itemmenu.rs:42-63`), carried out by `item_outs`
  (`play.rs:7455-7508`): use or equip (0x003E), destroy (0x003C), swap, quick slot, gear set (0x0021), link, hide
  look. The inventory is always held (`dump.inventory`, events at `play.rs:3783-3821`).
- **Today:** `inventory.get`, `items.*`, `inventory.changed`; `use_item`, `equip`, `swap_items`, `quick_slot`,
  `weapon_set`, `prepare_weapon_set`; `open_window("inventory")` (opens the Start menu and presses Enter).
- **Missing:** the diversion (kind `inventory`); actions `link_item` (`linkplay.rs:126`, public) and `hide_look`
  (`net/session.rs:2261`, public). Destroy stays unsupported.
- **Seams:** W1's two (1 and 2). **Risk:** the "waiting for the server" flag (`play.rs:555`) is reset only by the
  server's answer or by opening Items; a mod action must refuse while it is set.
- **Now easier:** the Bag Viewer already runs without the corpse, merchant and bank windows; this is one more
  kind on the same registry.

### 5.2 Corpse loot, merchant, bank (value: high; done in rounds 5 to 6.5)

- **Game:** modes 1, 10, 11 (`npcw.rs:160`, `:199`, `bankplay.rs:100`); `LootOut`, `MerchantOut`, `BankOut`.
- **Today:** complete: reads, `loot`, `loot_all`, `open_loot`, `talk`, `buy`, `sell`, `bank_move`, `bank_money`,
  takeover with `own` and `all` (`npcw.rs:164`, `:202`, `bankplay.rs:103`, `:188`).
- **Missing:** master loot (5.13). Selling on the auction from the bank page (5.26).
- **Round:** A0, helper mods only (auto-sell junk, restock, auto-bank). No API change.

### 5.3 Spellbook and abilities (value: high)

- **Game:** mode 5, set in `start_actions` (`play.rs:7730-7733`); requests `spellbook::Out`
  (`spellbook.rs:80-95`) carried out by `book_actions` (`play.rs:7682-7720`): memorize 0x0033, arrange 0x003A,
  unmemorize all 0x00FE. The server's slot question: `slot_asked` (`play.rs:7690-7692`), cleared by 0x00B4
  (`play.rs:3976-3980`).
- **Today:** `abilities.*`, `cooldowns.get`, `abilities.changed`; `cast`, `assign_ability`,
  `unmemorize_ability`, `unmemorize_all`, `arrange_ability`. Complete for a replacement.
- **Missing:** the diversion (kind `spellbook`); the slot question as a session change. **Seams:** W1's.

### 5.4 Hotbars, weapon sets, auto-cast (value: high)

- **Game:** six quick menus in carousel order (`hudmenu.rs:78-84`): up to four ability bars of five slots, items
  (four quick slots), weapon sets. A slot holds an ability, an item or a weapon set; there are no command or macro
  slots (`hudmenu.rs:50-56`). Fired through `use_hud_entry` (`play.rs:9127-9134`). No bar-editing window: abilities
  are placed from the spellbook, items from the item menu. Auto-cast marks a slot (`autoplay.rs:151`,
  `/autocast` at `play.rs:6477`).
- **Today:** `hotbar.list`, `hotbar.changed`, `use_hotbar`, `quick_slot`, `assign_ability`,
  `hud.hide("hotbar_draw_compact"|"hotbar_draw_expanded")`, the legacy weapon-set calls.
- **Missing:** which slots are on auto-cast (read) and a typed toggle (today only `command("/autocast ...")`); the
  hidden hotbar still toggles auto-cast on a long press (`play.rs:5907`).
- **Seams:** (1) optional guard line in `hud_hold`, shared with 5.9. **Open in the backlog:** the Forge layer still
  shows three bars; `eqoa.hotbar.list` already has all of them.

### 5.5 Quest journal and tracker (value: high)

- **Game:** journal mode 22 (`play.rs:7785-7788`); up to eight titles and one stamp (`questlog.rs:17-24`); the
  only request is Drop (`play.rs:8176`). The tracker is a HUD piece fed by `session.quest_progress`
  (`net/session.rs:1535`): per quest a name, goals with text, have and need, and an "ability quest" flag
  (`net/session.rs:936-950`). Servers that do not send 0x7E06 give titles only.
- **Today:** `quests.list/get` (title, index), `quests.changed`, `drop_quest`, `open_window("quests")`.
- **Missing:** the diversion (kind `quests`); **quest goals** (`Quest.goals`, `Quest.ability`) where the server
  sends them; hiding the tracker (seam 3).
- **Seams:** W1's for the journal; none for goals.

### 5.6 NPC dialogs (value: highest for helper mods)

- **Game:** plain text is mode 9: shown at once in play, else queued, eleven at most (`play.rs:1045-1053`,
  `:1204-1210`). Text with answers is mode 7: shown at once, else one waits, a further one is refused to the
  server (`play.rs:1108-1124`). An answer sends the dialog's id and the choice (`play.rs:1193`). Waiting ones open
  when play resumes (`play.rs:4393-4402`). A dialog carries an id, text and options and **no speaker**
  (`play.rs:1108`). A server may send more than six options (`dialog::PAGE`, `play.rs:1113-1115`).
- **Today:** `talk`, `interact`; the visible dialog through `windows.get` (text, options); `dialog_answer` on
  the focused native window only.
- **Missing:** kinds `text_dialog` and `response_dialog`, sessions with the text, options and the entity last
  talked to, `dialog_answer(session.ref, option)`, `pass`; the fork's own queue with the game's limits.
- **Seams:** none new (the packet hooks at `play.rs:3662` and `:4325`, plus seam 1 for dialogs from the queue).
- **Risk:** the largest in the plan. See the design's risk 2: no speaker, late answers, wrong-quest answers.
- **Now easier:** round 6.5 already detects a packet-opened window by comparing the mode before and after the
  packet and declines one that was open already; dialogs use the same test. Walk-away and timeout handling exist.

### 5.7 Yes/no prompts (value: medium)

- **Game:** all on mode 14, answered in `dialog_key` (`play.rs:1155-1183`): group invite (`play.rs:7268-7274`),
  summons or resurrection (`play.rs:7281-7293`, answer 0x00D4), pet dismiss (`play.rs:8707-8711`), quit
  (`play.rs:7735-7739`), guild disband and quit, send report, GM prompts. Separate modes: buddy request 17,
  guild invitation 20, trade request 16, duel invitation 43. The group invite has no timeout; the buddy request
  and guild invitation time out after 45 s, but only while their window is drawn (`social.rs:555`, `:684-687`;
  `guild.rs:654`, `:839-841`).
- **Today:** the visible prompt through `windows.get` (text, style, tag), `dialog_answer` on it.
- **Missing:** the prompt's kind (section 2); kinds `group_invite`, `revive_offer`, `summon_offer` and
  `actions.answer(session.ref, yes)`; for a hidden buddy request or guild invitation the fork must run the 45 s
  itself and refuse, or the request never ends. Quit, report and GM prompts are never claimable.
- **Seams:** seam 1 only.

### 5.8 Player, target, pet and group frames (value: high)

- **Game:** HUD pieces drawn by `Hud::draw` (`hud.rs:407-530`): player `:414-433`, group `:533`, pet `:574`,
  target `:611`. The target frame has no click area. The target detail box is drawn apart, not as a piece
  (`play.rs:10478-10501`). The breath gauge is its own piece (`play.rs:10470-10477`).
- **Today:** hide tokens for health, power, both experience bars, compass, target, group panel, group marks, pet,
  effects, hotbar and chat (`fork_ext/addons/visibility.rs:14-29`) and the minimap; `hud.layout`, `hud.frame`,
  `hud.text/gauge/image/panel`; `player.*`, `target.get`, `entities.*`, `group.get`.
- **Missing:** hiding the raid list, tracker, DPS meter, breath gauge and the stacked spell bars; click areas of
  hidden pieces (the player frame's is noted before any check, `hud.rs:416`); reads for breath, the target's
  title and guild (`play.rs:11067-11084`), consider colour (`play.rs:3668-3673`), the player's Hardcore flag.
- **Seams:** 3, 4, 5, 6 (four lines); (1) for the target detail box.

### 5.9 Buffs and effects (value: high)

- **Game:** two sources. The game's strip: up to eight effects with icon and name (`net/dump.rs:202-206`). The
  buff window, where the server offers it: per line whose it is (player or pet), **removable**, spell id, icon,
  **time left in ms**, name (`net/buffs.rs:36-49`); removal is op 0x7E59 through the public
  `Session::send_buff_remove` (`net/session.rs:2198`). Pet effects arrive by 0x7E33 (`net/session.rs:3306`). The
  client holds no caster, no stacks and no buffs on other players or the target.
- **Today:** `effects.list` (`icon`, `index`, `name`), `effects.changed`, `hud.hide("active_effects")`.
- **Missing:** `Effect.time_left`, `removable`, `whose`, `spell_id` (nil where the server sends none); pet
  effects; `actions.remove_effect(ref)`.
- **Seams:** none for the data and the action; (1) guard so a hidden buff window stops taking long presses.

### 5.10 Cast bar (value: high)

- **Game:** none. The client knows its own cast window from 0x00B1 (`play.rs:4236-4240`) and receives every
  nearby cast with caster, time and effect (`play.rs:4233-4243`, `spells.rs:199-209`).
- **Today:** nothing. `cooldowns.*` starts after the cast.
- **Missing:** event `cast.started {caster, ability?, effect, duration}` and `player.casting`. With it a mod
  draws a cast bar for the player and for the target. **Seams:** none (`cast_window` is private to `play.rs`).

### 5.11 Pets (value: medium)

- **Game:** `pet_id` from `Event::Pet` (`play.rs:3962-3967`); the pet bar and its effects row (`hud.rs:574-600`).
  Commands are one opcode with a byte, sent by the private `pet_command` (`play.rs:8696-8721`), reached from the
  Start menu and quick chat; dismiss asks first.
- **Today:** `player.has_pet`, `player.pet_id`, the pet as an entity, `hud.hide("pet_panel")`.
- **Missing:** `actions.pet(command)`. The command bytes carry no names in the code (0 to 4, 7, 8 with a target,
  5 after the dismiss prompt); name them from the Start menu's words in the round. **Seams:** none.

### 5.12 Group (value: high)

- **Game:** `Play.group` is always held (`play.rs:489`, `group.rs:30-44`). `group_action` (`play.rs:8726-8762`):
  invite the target (0x0625), remove (0x0626), disband (0x0627), leave (0x0623). Accept and decline:
  `accept_invite`, `decline_invite` (`play.rs:7346-7368`). There is no client command to pass leadership of a
  plain group. The loot rule exists only under the Raid menu (`raidplay.rs:152-157`).
- **Today:** `group.get` (members, invites, raid rows, `self_leader`), `group.changed`, `target`, `assist`,
  `hud.hide("group_member_panel")`.
- **Missing:** `actions.group_invite(entity|name)`, `group_remove`, `group_leave`, `group_disband`,
  `group_accept(invite)`, `group_decline(invite)`. All call private functions of `play.rs`. **Seams:** none.

### 5.13 Raid (value: medium; high for raiders)

- **Game:** the raid list shares the Group piece (`hud.rs:440-470`, `raidlist.rs:381`, `:507`), with its own
  wheel (`hud.rs:371`) and row dragging (`raidplay.rs:359-401`). Commands are Start menu entries handled by
  `raid_action` (`raidplay.rs:271-331`): loot rule, leave, invite, make leader, helper or main assist, kick, move
  to group. Ready checks and loot rolls are strips drawn in play (`raidplay.rs:546-553`), answered by private
  `make_ready` and `make_roll` (`raidplay.rs:213`, `:244`), with deadlines (`rollwnd.rs:115-120`,
  `readywnd.rs:84`). Master loot lives inside the corpse window (`masterloot.rs:80-94`, `npcw.rs:494-499`).
- **Today:** raid rows in `group.get` (id, name, health, power); `hud.hide` misses the raid list
  (`hud.rs:440-470` has no check); item actions refuse while a master-loot choice is open
  (`fork_ext/play.rs:2914`).
- **Missing:** a member's group number, role and mark; typed raid commands; kinds `raid_ready` and `raid_roll`
  with choices and deadline; `loot_assign(item, member)` and `loot_abandon(item)` for the master looter.
- **Seams:** 3, 4, 5 are shared with 5.8; 7 to 10 (four lines) for the strips; the meter's wheel is seam 6.
- **Risk:** a missed deadline is a lost roll, so a strip passes back to the game when its holder is gone.

### 5.14 Chat (value: high)

- **Game:** the box is a HUD piece drawn through an existing fork hook (`play.rs:10384`); the entry line is the
  shared edit box, mode 2 (`play.rs:8278-8287`), which also serves buddy, guild, ignore, alarm and report text;
  history is mode 33. Channels: say, group, guild, shout, tell, reply (`play.rs:6432-6526`). Tabs and their line
  kinds are the player's settings (`settings.rs:1098-1101`, `chat.rs:426`). Item links travel with a line
  (`play.rs:3666`, `linkplay.rs:126`).
- **Today:** `chat.history`, `chat.message`, `chat.get_channel`, `actions.chat/command`, `hud.hide("chat_window")`,
  `input.capture_text`. The three bundled UIs already replace the box.
- **Missing:** kind `chat_entry` (the game's chat keys open the mod's box); the tab setup and a line's kind as
  reads; the links in a line and `link_item`; the last tell's sender (`play.rs:468`); `set_channel`.
- **Seams:** seam 1. **Risk:** mode 2 is shared, so only the chat purpose (`0x0B0F_0000`) may be diverted;
  password and report text never enter a session.

### 5.15 Start menu (value: medium)

- **Game:** mode 13 (`play.rs:7416-7426`). Entries come from the game's own tables on the disc
  (`startmenu.rs:1007-1026`) plus client-added ones (duel, combat record, raid, dismiss mercenary), filtered at
  each open by server features (`play.rs:7587-7631`). An entry runs through `start_actions` (`play.rs:7724-7800`)
  or `do_action` (`play.rs:8516`). Many windows return to it when they close.
- **Today:** `open_window("start_menu")`, its visible rows, `menu_command` on the visible native menu.
- **Missing:** kind `start_menu` with the filtered entry list as session data, and `menu_command(session.ref,
  command)`. The list is built inside the menu's draw; the fork can run that draw into nothing, as
  `bank_taken_tick` does for the bank. **Seams:** seam 1.

### 5.16 Character summary and stat training (value: medium)

- **Game:** mode 12 (`play.rs:7771-7776`). Training points are `dump.player.p40`, set by 0x0036
  (`play.rs:1061-1082`); the stat cap is `p44` (`play.rs:1086-1092`). Spending is one request,
  `trainer::train_stats` (public, `trainer.rs:681-691`), refused when the total exceeds the points.
- **Today:** `player.stats()`, `player.stats_changed`, `open_window("summary")`; the window's rows.
- **Missing:** **training points are not exposed** (the cap is); `actions.train_stats({...})`; kind `summary`.
- **Seams:** seam 1. **Risk:** points spent cannot be taken back; the action reports `sent`, never `confirmed`.

### 5.17 Class masteries (value: medium)

- **Game:** mode 40 (`play.rs:7756-7762`); `classmastery::Part` with public requests (`classmastery.rs:376-414`),
  one pending at a time, 10 s give-up. The skill list is fetched at open and cleared at close; the buy and switch
  confirmations live in the window (`classmastery.rs:881-931`).
- **Today:** `mastery.get/skills/selected`, `mastery.changed`, `mastery_share`, `mastery_browse`, `mastery_buy`,
  `mastery_refund` (the last three only with the native window open).
- **Missing:** the same on a taken session; the Master Class rule and both confirmations become the mod's job.
- **Seams:** seam 1, plus 1 at `play.rs:7950` so an inspected skill survives without mode 40.
- **Backlog tie-in:** "XP to the next Class Mastery point" is a read on `mastery.get` plus a HUD line.

### 5.18 Factions, combat record, DPS meter (value: low)

- **Game:** factions mode 41, fetched per page at open (`faction.rs:37-59`, `play.rs:7764-7769`). Combat record
  mode 60, pushed by the server after one request (`recordplay.rs:48-70`, `net/session.rs:1950`). The DPS meter is
  a HUD piece the server fills (`play.rs:491`, `:3721-3727`), with `/dps` (`play.rs:10903`).
- **Today:** `open_window("factions")`; `combat.hit` for a mod's own meter. Nothing reads the three.
- **Missing:** `factions.list(page)`, `combat.record()`, `combat.meter()`; hiding the meter (seams 3 and 6).
- **Seams:** none for the reads.

### 5.19 Friends and ignore (value: medium)

- **Game:** both lists are always held (`play.rs:646-649`, `social.rs:86`, `:386`). Windows: buddy request 17,
  buddy list 18, ignore list 26. All wire builders are public (`social.rs:132-147`, `:427-464`).
- **Today:** `open_window("friends"|"ignore")`.
- **Missing:** `social.friends()`, `social.ignored()`, events, `friend_add/remove`, `friend_accept/refuse`,
  `ignore_add/remove`; kinds `friends`, `ignore`, `friend_request`. **Seams:** seam 1 only.

### 5.20 Guild (value: medium)

- **Game:** `Play.guild` is public and always held: name, message of the day, members with rank
  (`guild.rs:69-77`, `play.rs:657`). Roster mode 21, invitation mode 20. Commands are one opcode with public
  builders (`guild.rs:270-329`): invite, promote, demote, kick, set the message, guild say, create, disband,
  quit, join, refuse.
- **Today:** `open_window("guild")`; guild chat through `actions.command("/gu ...")`.
- **Missing:** `guild.get()` (roster, ranks, message), `guild.changed`, the commands, kinds `guild` and
  `guild_invite`. Disband and quit keep the game's prompt (owner question 3). **Seams:** seam 1 only.

### 5.21 Who, player search, group search (value: medium)

- **Game:** who list mode 35, fetched at each open with a serial (`wholist.rs:57-93`). Player search is an overlay
  on servers with the Sandstorm patch (`searchwnd.rs:411`, `play.rs:8566-8581`). Group search (looking for group)
  is mode 42 on servers that offer it (`liveplay.rs:165-194`); its builders are public (`groupsearch.rs:85-104`).
- **Today:** `open_window("who"|"search")` (D4).
- **Missing:** `who.request()` with a `who.result` event, `lfg.search/register/unregister`, invite and tell from
  a row (both already exist as group invite and chat). **Seams:** seam 1 only.

### 5.22 Crafting (value: medium; on the owner's helper list)

- **Game:** recipe book mode 6, experiment mode 37 (`play.rs:8103`, `:8107`). `Craft` has public state and
  requests (`craft.rs:208-360`): create, remove, move and inspect a recipe; add to and remove from the tray (four
  items at most, `craft.rs:626`); combine (`craft.rs:322`). After a combine the server may ask whether to keep the
  recipe: a yes/no whose answer value lives in the window (`craft.rs:594-613`, `craftwnd.rs:726-730`).
- **Today:** `open_window("recipes"|"experiment")`.
- **Missing:** everything else: a `craft` read (skills, recipes, detail, tray, pending), the actions, the
  server's question as a session change, kinds `recipes` and `experiment`.
- **Seams:** seam 1, plus 1 at `play.rs:3865`, where replies are read only if mode 6 or 37 is current. The
  design counted none.

### 5.23 Trade between players, duels (value: medium)

- **Game:** trade request is mode 16, the trade window mode 15 (`npcw.rs:330`, `:384`); `TradeOut`
  (`trade.rs:285-303`): add, remove, money, accept, unaccept, cancel. A request arriving with any window up is
  answered "busy" by the client (`npcw.rs:332-338`). Duels: invitation mode 43, wager mode 44, which reuses the
  trade window (`duelplay.rs:133`, `:152`); every request echoes a duel id (`duel.rs:148-176`).
- **Today:** nothing; the modes are named only.
- **Missing:** reads, actions and kinds for both. **Seams:** 2 (child-module lines in `npcw.rs` and
  `duelplay.rs`; the functions are private there).
- **Risk:** high. Items and money change hands; a hidden trade must never accept on its own.

### 5.24 Blacksmith (value: medium)

- **Game:** mode 19, opened by the server from an NPC (`support.rs:168-178`); repair one item (0x0062), close
  (0x0063); the cost is computed by the client (`blacksmith.rs:28`). No repair-all.
- **Missing:** kind `blacksmith`, `repair(item)`, the cost as a read. A "repair all" helper is then a mod.
- **Seams:** 1 (`support.rs`: the smith's id is private).

### 5.25 Mail and tells (value: low)

- **Game:** compose 23, inbox 24, tell form 28, reply form 29 (`play.rs:7791-7794`); the inbox is fetched at open
  and holds 100 entries (`gram.rs:1475-1491`); builders are public (`gram.rs:265-308`).
- **Today:** `open_window("mail"|"mail_compose")` (D3); tells through `actions.chat`.
- **Missing:** `mail.inbox()`, read, delete and send; kinds. **Seams:** (1) if the request state in `mail.rs`
  must be driven from outside.

### 5.26 Auction (value: medium)

- **Game:** search is mode 39 (`bankplay.rs:53-56`), opened from Start menu entries that exist only while the
  bank offers the auction (`play.rs:7591`); selling is a page of the bank window. Results are paged and dropped
  when the mode is not 39 (`auction.rs:166-197`, `:354-357`). Bid, buyout and sell each confirm in the window.
- **Today:** nothing (D1, D2). **Missing:** all of it.
- **Seams:** 1 (`bankplay.rs` or `auction.rs`). **Risk:** high: money, paged replies, confirmations.

### 5.27 Mercenaries, dye picker, fishing (value: low)

- **Mercenaries:** windows 61 and 62 are opened by the server (`mercplay.rs:150-187`); dismiss is a Start menu
  entry (`mercplay.rs:56`). Hiring is not in the client. A hidden bank or corpse session leaves the game in mode
  0, so these windows can open over it (backlog, 2026-11-18). 1 line (`mercplay.rs`).
- **Dye picker:** mode 63, opened by the server with a token (`dyeplay.rs:185-203`); 1 line (`dyeplay.rs`).
- **Fishing:** mode 36, opened by the server (`fishplay.rs:86-95`); cast, reel, keep, pull, cancel have public
  builders (`fishing.rs:129-176`); the cast's power meter is in the window. (1) line.
- **Today:** none of the three is named for mods beyond fishing's mode.

### 5.28 Name plates, damage numbers, target rings (value: high for an ElvUI-style look)

- **Game:** name plates are built by `Play::plates` (`play.rs:5079-5097`) and drawn by the viewer; damage numbers
  by `Damage` (`damage.rs:74`, `play.rs:10147-10150`); rings by `draw_rings` (`play.rs:5041`). None is a HUD
  piece, none can be hidden by a mod.
- **Today:** `combat.hit`, `entities.*`, `camera.get`.
- **Missing:** a world-to-screen projection (`eqoa.camera.project(position)`) so a mod can place its own plates
  and numbers; hide switches for the three. **Seams:** (3), one guard line each.

### 5.29 World map and minimap (value: low; already mod-friendly)

- **Game:** `map/mod.rs:378` show, `:393` close, `:445` keys.
- **Today:** `map.get/open/markers/view/set_guide`, `hud.hide("minimap")`; the Round Minimap and Map Search mods.
- **Missing:** kind `map`, a full-size terrain layer, `set_waypoint/clear_waypoint` (described `not_yet`).
- **Seams:** seam 1, (1) for terrain.

### 5.30 Emotes, jump, death, experience, Hardcore (value: low)

- **Emotes:** four, sent by `do_action` (`play.rs:8599-8604`); no window. Missing: `actions.emote(name)`.
- **Jump:** `jump_key` (`play.rs:6799-6803`), where the server offers it. Missing: `actions.jump()`.
- **Death:** no window and no release button; a bar and a fade drawn inside the HUD (`play.rs:10332-10338`,
  `:10558-10563`); respawn is the server's. `player.dead` exists. Nothing to replace.
- **Experience:** `player.experience`, `experience_debt`, both bars hideable. Level-up is a sound only
  (`play.rs:9183-9189`); a `player.level_up` event is a small addition.
- **Hardcore:** chosen at creation, shown as a memorial on character select, a fall text in the world
  (`play.rs:10565-10573`). Missing: a `player.hardcore` flag, so a helper can refuse risky automation.

### 5.31 Options, keys, macros (keep native)

Options is mode 34 with compile-time pages (`options.rs:1793`, `:2036-2085`); the fork appends its own page
through existing hooks. Mods add switches, buttons and keys with `eqoa.options.*`, `eqoa.binds.*`,
`eqoa.input.bind`. Macros are pad macros kept per computer (`binds.rs:419`, `macros.rs:76`). The owner already
decided options are never replaceable. Open: key conflicts (round 7 Part E) and one home for the fork's options
(backlog, round 8).

### 5.32 Login, character select, creation (not proposed)

The screens before the world are a separate program flow (`pregame/`), with no fork hook at all. Character select
shows name, level, class, race, masteries and the Hardcore memorial, no zone (`pregame/client/mod.rs:84`,
`select.rs:44`). The mod runtime already draws on those screens with an empty game state (`viewer.rs:4052`), so a
mod can show its own window before the world, but it cannot read the character list or act. Replacing these screens touches login and
deletion and would need new seams in a ported system. Proposed: leave them; at most a read-only character list
later, on request.

## 6. Upstream lines, counted

All are inserted one-line calls. Nothing upstream is moved, deleted or rewritten.

| Group | Lines | Where | Status |
|---|---:|---|---|
| Round 6.5 (built) | 4 | `npcw.rs:164`, `:202`; `bankplay.rs:103`, `:188` | in |
| W1 diversion | 2 | first line of `draw_hud` (`play.rs:10185`); in the window pass before the Start menu and Items draw (`play.rs:10450`) | firm |
| HUD completion | 4 | `hudlayout.rs:408` (`end`); `hud.rs:388` (`note`); `hud.rs:371` (`raid_wheel`); `play.rs:10796` (`meter_wheel`) | firm |
| Raid strips | 4 | `raidplay.rs` module line; `:546` `run_rolls`; `:454` `roll_key`; `:519` `roll_point` | firm |
| **Firm total** | **10** | | as the design |
| Mode-checked replies | 2 | crafting `play.rs:3865`; class mastery `play.rs:7950` | probable |
| Private to a file | 6 | `npcw.rs` (trade), `duelplay.rs`, `support.rs` (blacksmith), `bankplay.rs` (auction), `mercplay.rs`, `dyeplay.rs` | probable, one per window when asked for |
| **Probable total** | **18** | | |
| Optional guards | 9 | long-press on hidden buffs or hotbar (2, `play.rs:5902`, `:5907`); target detail box (1); name plates, damage numbers, rings (3); map terrain (1); fishing (1); mail (1) | each decided in its round |
| **Ceiling** | **27** | | the design said 22 |

Zero-line work, on hooks that exist: every read in section 5, group, pet, guild, friends, emote, buff and
training actions (they call `play.rs`'s own private functions or public session calls), packet-opened windows
(`play.rs:3662`, `:4325`), pad focus (`viewer.rs:3248`), keyboard and pointer (`viewer.rs:5480`).

## 7. Roadmap after round 7

Small rounds, each ending with something the owner can test. The W numbers are the window design's. Each round
keeps the rule: the owner approves the five-line "what you'll see" first, one example mod or option proves it.

**Before the freeze (round 7, not a new round):** defects D1 to D4; the three contract rules (unknown kind
`unsupported`, `all` needs `replaces`, `own` refused on views); names for modes 61 to 63; the prompt's kind; the
owner's answer on servers that ask for the game as it was (question 1).

| Round | Content | Lines | The player can check | What is easier now that takeover exists |
|---|---|---:|---|---|
| A0 (W0) | Helper mods on 6.5 as built: auto-sell junk, restock to N, auto-bank marked items | 0 | Each helper does its job; opening the merchant by hand still shows the game's window | Everything: hidden sessions, `open_loot`, `talk`, batches and the "can act on its own" label are built and owner-tested. These are Lua only. |
| A1 (W1) | The contract (session, `pass`, `focus`, `replaces`, label, "use the game's windows" switch); bags, spellbook, journal; `link_item`, `hide_look` | 2 | The bag key opens the mod's bags; disabling the mod gives the game's bags back at once | The registry, one holder per kind, `busy`, release on disable and the capability gate are reused as they are. New work is the session object and the two lines. |
| A2 (W2) | NPC dialogs, group invitation, summons and resurrection offers; label line "Can answer NPC dialogs"; example: a dialog window and a turn-in helper | 0 | Hail an NPC, read and answer in the mod's window; an unknown dialog shows as the game's | The "was it already open before this packet" test, the walk-away and idle timeouts and the `takeover:` log lines carry over. `pass` from A1 is what makes a wrong match harmless. |
| A3 (new) | Frame data, no windows: buff time left and removable, pet effects, cast events, quest goals, training points, breath, target title and consider colour, raid member group and role; actions for group, pet, remove buff, emote, jump | 0 | A demo frame mod shows buff timers and a cast bar; a button invites the target; a button sends the pet in | No takeover needed. The action broker, capability names and the Play-shaped test fixtures from rounds 6 and 6.5 are the whole base. |
| A4 (W5) | HUD completion: hide every one of the 12 pieces and their click areas; raid list; ready-check and roll strips as sessions; master-loot actions | 8 (+3) | Hide the target frame and click where it was: nothing happens. Answer a ready check in a mod | Strips become two more kinds on the registry; passing a strip back when its holder is gone is A1's `pass`. |
| A5 (W3) | Character summary with training; class mastery on a session; factions and combat record reads | 0 to 1 | Spend one point and buy one skill from a mod window | Views are the simplest sessions: no server conversation to hide. Mastery's typed actions exist; only their "window must be open" rule changes. |
| A6 (W6) | Start menu as a session; chat entry; chat tabs, line kinds and links | 0 | The Start key opens a mod menu; Enter types into a mod chat box | `focus` from A1 gives the mod the pad; the bank's "run the window into nothing" trick builds the entry list. |
| A7 (new) | Community: friends, ignore, guild, who, group search: reads, actions, request prompts | 0 | A mod friends list; accept a guild invitation in a mod | The data is always held and the builders are public, so this is reads plus actions. The 45 s request timers reuse the hidden-session timeout. |
| A8 (W4) | Crafting reads and actions; the server's "keep this recipe" question | 0 to 1 | Browse recipes and do one combine from a mod window | A server question inside a session is the same shape as A2's dialogs. |
| A9 | Trade, duel, blacksmith | 3 | Repair from a mod; trade one item in a mod window | Server sessions like merchant and bank: `own` and `all` scopes, never replaying an uncertain send. |
| A10 | Mail and auction | 1 (+1) | Read mail and search the auction in a mod | Paged results and pending states follow the bank's pending-move handling. |
| A11 | Mercenary windows, dye picker, fishing | 2 (+1) | A mod mercenary bag | Also settles the backlog item about these windows opening over a hidden session. |
| A12 (new) | World overlays: `camera.project`, hide name plates, damage numbers and rings | (3) | Mod-drawn name plates over heads | Nothing from takeover; the HUD hide tokens are the model. |
| A13 (W7) | World map session and terrain layer; `set_waypoint` | 0 (+1) | A mod-drawn map opens on the map key | A view like the others. |

**Changes from the design's order, and why.** A3 is new and comes third: it costs no upstream line, needs no
contract beyond what exists, and gives unit-frame mods what they lack most (timers and a cast bar). HUD completion
moves up from W5 to A4 because hiding is the other half of a frame replacement. Character and mastery (W3) and
crafting (W4) move back: the audit found one mode-checked reply in each, and fewer players ask for them. The long
tail (W8) is split into A7 and A9 to A11, ordered by how much of each is zero-line work. A12 is new.

**Scale.** About 35 new takeover kinds, about 45 new typed actions, about 25 new reads or record fields and about
8 new events, in 14 rounds. A0 to A4 are roughly a third of the work and most of the value; A5 to A8 another
third; A9 to A13 the rest, built only as mods ask for them. All of it is additive under `eqoa/1` once the three
contract rules and the four defects are settled.

## 8. Risks

1. **Hidden conversations with the server** (A2, A9, A10). A dialog names no speaker; trades and auctions move
   items and money. A mod that answers wrongly accepts the wrong quest or the wrong trade. Limits: `all` plus
   `pass`, matching on the dialog's own text, never replaying an answer, no accept without the mod's explicit
   call. Only the owner's live test settles how the server treats late or missing answers.
2. **Getting stuck.** A replaced window no longer holds the controls; a mod that takes the pad and stops drawing
   locks the player out. Focus ends on error, disable and focus loss; Mods & Addons and the "use the game's
   windows" switch stay native.
3. **Drift from upstream.** Typed actions call the game's functions, not its windows. When upstream changes what
   a button does, the action can fall behind without a compile error. Upstream moves fast: 133 commits in the last
   sync, three new modes. Each window gets a test that presses the native button and calls the mod action on the
   same state; the sync review reads the window files' diffs.
4. **Replies checked against the mode** (section 3.2). Missed, a hidden craft or auction search waits forever.
   Each such window needs its line and a test with the native window closed.
5. **Servers that ask for the game as it was.** Until the owner decides, a mod can show there what the server
   switched off. That may cost the client its welcome on those servers.
6. **Freezing with the four defects in.** After the freeze, fixing what `open_window("mail")` opens is a
   breaking change by the plan's own rule.
7. **One holder per window.** A UI skin and a turn-in helper both want the NPC dialog; the second is refused by
   name. More kinds make this more common. Chaining holders is possible later, not planned.
8. **Timers that only run while a window is drawn** (buddy request, guild invitation, rolls, ready checks). A
   hidden one must be timed by the fork or the other player waits.
9. **Footprint.** The ceiling rises from 22 to 27 lines. Each optional line is small, but every one is a place a
   sync can conflict; the firm 10 are the ones worth offering upstream in round 8.

## 9. Owner questions

1. **Servers that ask for the game as it was** (Vanilla Test, [OG] accounts): should mods obey the server's
   switches there? Proposed: yes. The reads behind a switched-off extra answer `unavailable` and acting on its own
   is off; Mods & Addons says "limited on this server". Settle before the freeze.
2. **Order:** A0, A1, A2 as already approved, then the new A3 (frame data, no upstream lines) before HUD
   completion. Agreed?
3. **Irreversible steps** (guild disband or quit, pet dismiss, mail delete, spending training points): a mod
   action opens the game's own yes/no instead of skipping it. Agreed? Destroy stays unsupported either way.
4. **Before the world** (login, character select, creation): leave to the game. Agreed?
5. **Name plates and damage numbers** (A12): wanted? They cost up to 3 upstream lines of their own.

## Not verified

- No build, test or live play; server behaviour (range, late answers, order of replies) is known only from the
  owner's round 6 and 6.5 tests.
- Pet command meanings (which byte is attack, back off and so on), and the Start menu's labels and paths: they
  come from the game's own tables on the disc, not from code.
- Whether the client filters an ignored player's chat itself.
- How quick-chat sets load, and whether `mail.rs`'s request state can be driven without a line there.
- Whether every HUD piece's text lands in the quad list seam 3 trims (the design's open check).
- The eight "probable" lines are counted from which functions are private; a round may find a zero-line route
  (as round 6.5 did for its review fixes) or need one more.
- The ImGui bridge and the Forge layer were not re-audited here; `round-7-audit.md` covers them.
