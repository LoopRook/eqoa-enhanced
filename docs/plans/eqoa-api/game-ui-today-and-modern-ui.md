# The game's UI today, and the modern UI built from mod windows

Study, written 2026-11-22 on `docs/game-ui-study` at `05fe75ee` (develop with the 2026-11-21 upstream sync). No
game code was changed; nothing was built or played. It answers the owner's request: know how the game's UI works
now, and what a mod can do with each part of it.

**The owner's design (2026-11-22).** Each mod window is its own window on its own key, as in a modern MMO: one
key for bags, one for the spellbook, one for the journal. The game's main menu (the Start menu) stays as it is;
replacing its pages is not the goal. Game keys win over mod keys (game control, then macro, then mod), so a mod's
default key must be a free key, and the player can change it on the Controls page.

**Reading this.** Paths are under `crates/eqoa/src/`. Every line number was read on the baseline above; anything
not read there is marked *(not verified)*. Round names (A0 to A13) and counts of upstream lines are those of
[ui-coverage-audit.md](ui-coverage-audit.md) section 7 and were not recounted. That audit and
[window-replacement-design.md](window-replacement-design.md) carry older line numbers.

## 1. How the game's UI works today

The game is in one **mode** at a time: 0 is play, and each window is a mode number (list:
`fork_ext/addons/window_kinds.rs:8-55`). Only play takes the movement keys; any window up takes every key
(`play.rs:7046-7125`). Nearly every window is reached through the **Start menu**, and closing one returns to the
Start menu, not to the world. Only the map, the chat line, quick chat and Options' Controls page have a key. The
Start menu's own labels and layout come from tables on the disc, not from code *(not verified)*.

### Start menu and what it opens
| Element | Opens by | Closes by | Code |
|---|---|---|---|
| Start menu | Escape, or Start on the controller | Escape or △; choosing an entry | `viewer.rs:5043`, `play.rs:7239`, `:7446`, `:7690` |
| Bags and worn gear ("Items") | Start menu > Items. A page inside the Start menu, not a window of its own; no key | Back returns to the Start menu's main page | `play.rs:7685-7689`, `:7676-7680` |
| Spellbook ("Abilities") | Start menu entry; no key | Back reopens the Start menu | `play.rs:7760-7764`, `:7742-7747` |
| Quest journal | Start menu entry, or a click on the quest tracker; no key | Back: Start menu (the world, if opened from the tracker) | `play.rs:7815-7818`, `:6040-6045`, `:8205-8210` |
| Character summary, stat training | Start menu entry | Back: Start menu | `play.rs:7802-7806`, `:8092` |
| Class mastery; factions | Start menu entries (each asks the server for its list) | Back: Start menu | `play.rs:7786-7800`, `:8007`, `:8040` |
| Crafting (recipe book, experiment) | Start menu entries | Back: Start menu | `play.rs:7809-7813`, `:8171` |
| Options | Start menu entry; F1 opens its Controls page from play | Back: Start menu *(not verified)* | `play.rs:7774`, `viewer.rs:5124` |
| Friends, ignore list, guild roster | Start menu entries | Back: Start menu | `play.rs:7775-7776`, `:8647-8653`, `:9827` |
| Mail (compose, inbox), tell and reply forms | Start menu entries | Back: Start menu | `play.rs:7821-7824`, `mail.rs:240` |
| Who list, player search, group search | Start menu entries (search only on servers that offer it) | Back: Start menu | `play.rs:8590-8614`, `liveplay.rs:186-187` |
| Auction search | Start menu entry, only while the bank offers the auction | Back: Start menu | `play.rs:8575`, `bankplay.rs:53-67` |
| Support request and inbox | Start menu entries; F3 on a server that takes bug reports | Back: Start menu | `play.rs:7826-7827`, `viewer.rs:4402` |
| Combat record, duel, raid commands; quit (a Yes/No box) | Start menu entries | per window | `play.rs:8556-8567`, `:7765-7770` |
| Pet commands, emotes, group commands | Start menu entries, no window; F5 to F8 are the group commands | - | `play.rs:8576`, `:8633-8639`, `viewer.rs:5353-5356` |

### Windows the server opens
| Element | Opens by | Closes by | Code |
|---|---|---|---|
| Corpse loot | Attack key (G, ✕) on a corpse asks the server; its answer opens the window, only from play | Server's close; Back *(key not verified)* | `play.rs:6352`, `npcw.rs:153-173` |
| Merchant, bank, blacksmith | Talk (Q, □) on the NPC asks the server; its answer opens the window, only from play | Back, or the server | `play.rs:1234-1238`, `npcw.rs:193-202`, `bankplay.rs:95-103`, `support.rs:168-178` |
| NPC text (no answers) | Server packet. Shown at once in play, else queued (11 at most) | Enter or Escape | `play.rs:1049-1057`, `:1208-1221`, `:1153-1158` |
| NPC dialog with answers | Server packet. In play at once; else one waits and a further one is refused | Choosing an answer sends it | `play.rs:1112-1128`, `:1189-1200` |
| Trade request, trade window | Start menu > Trade on a target, or another player's request (answered "busy" if any window is up) | Accept, cancel, or the other side | `play.rs:8627`, `npcw.rs:324-389` |
| Duel invitation and wager | Server | answer | `duelplay.rs:129-152` |
| Fishing; mercenary bags and spells; dye picker | Server | per window | `fishplay.rs:86-89`, `mercplay.rs:146-180`, `dyeplay.rs:185` |

### Prompts
| Element | Opens by | Answered by | Code |
|---|---|---|---|
| Yes/No box: group invitation, summons, resurrection, pet dismiss, quit, guild disband | Server packet or a menu entry | Left/right, Enter; Escape is No | `play.rs:7298-7323`, `:8746`, `:1159-1188`, `dialog.rs:371-387` |
| Friend request; guild invitation | Server (refused as "busy" if a window is up) | their own small windows | `play.rs:9483-9501`, `:3892-3899` |
| Raid ready check and loot roll strips | Server; drawn over play, under every window | Hold C or R2, right arrow, then Enter; or a click | `raidplay.rs:202`, `:233`, `viewer.rs:5272-5274`, `play.rs:5941`, `:10451` |
| "Destroy item?" and "Drop quest?" | Destroy in an item's menu (asked for every item, starting on No); ✕ on a quest in the journal | Yes or No | `itemmenu.rs:1108-1116`, `questlog.rs:5`, `:199` |

### Chat, map and the HUD
| Element | Opens or shows by | Closes or hides by | Code |
|---|---|---|---|
| Chat line | Enter, R3, or a click on the chat box; `/`, T (tell), R (reply) start it with text | Enter sends, Escape cancels | `viewer.rs:5095`, `:5203-5213`, `play.rs:7256`, `:6011-6013` |
| Chat history (scroll back) | Hold C or R2 | Let go | `viewer.rs:5255`, `play.rs:7260-7266` |
| Quick chat menus | Z or L2 | Z or L2 again | `viewer.rs:5118`, `play.rs:8925-8929` |
| World map | M, a click on the compass or the minimap | M or Escape | `viewer.rs:5131`, `play.rs:5981-5983`, `:6033-6036`, `map/mod.rs:393` |
| Minimap; quest tracker | Always on; N and U turn them off and on | N; U | `viewer.rs:5144`, `:5152` |
| Target details box | Hold Q or □ on a target | Let go | `play.rs:10539-10559` |
| 13 movable HUD pieces: player frame (portrait, compass, health, power, experience), group bars or raid list, pet, target, minimap, quest tracker, DPS meter, effects (or buff window), chat box, breath gauge, quick menus, spell bars, utility bars | Always on in play. H hides the whole HUD | Move HUD moves each | `hudlayout.rs:36-70`, `hud.rs:517-623`, `play.rs:10411-10624`, `viewer.rs:5137` |
| Not movable pieces: name plates, target rings, damage numbers, "You have died" bar | Drawn in the world or over the HUD | - | `play.rs:5084`, `:5046`, `:10353`, `:10391-10397` |
| Mods & Addons (the fork's own) | F10 | F10 | `fork_ext/bindpage.rs:46` |
| Login, character select, creation | Before the world; a separate part of the program (`pregame/`) | - | *(not verified here)* |

## 2. How a mod replaces each element in the modern UI

There are four ways, and each element uses one:

- **Own window on its own key** for what the player opens: bags, spellbook, journal, character, crafting, friends.
- **Hide and redraw** for HUD pieces: the mod hides the piece and draws its own (`eqoa.hud.hide`).
- **Hidden session** for what the server opens: the game's window stays away and the mod shows its own window
  with the same content. This is how the Bag Viewer already handles corpse, merchant and bank.
- **Prompt answered in a mod**: the question appears in the mod's style; the player still answers it.

| Element | Way | Round | Exists in `eqoa/1` today | Missing, or cannot be replaced |
|---|---|---|---|---|
| Bags and worn gear | own window + key | A1 | Reads; use, equip, swap, quick slot, weapon sets; a mod key row. The Bag Viewer exists, with no default key (`bundled-mods/eqoa_bag_viewer/main.lua:100`) | Destroy (with confirm), link item, hide look |
| Spellbook | own window + key | A1 | Reads; cast, assign, arrange, unmemorize | The window itself (no bundled mod yet); the server's "which slot?" question |
| Quest journal | own window + key | A1 | Reads (titles) | Drop quest is `not_yet` until the confirm exists (`docs/modding/SUPPORT-TABLE.md:57`); quest goals are A3 |
| Corpse, merchant, bank | hidden session | done; A0 helpers | Complete (`npcw.rs:164`, `:202`, `bankplay.rs:103`) | Master loot (A4) |
| NPC text and dialogs | hidden session | A2 | Read and answer only on the game's visible window | Sessions, "pass back to the game" |
| Group invitation, summons, resurrection | prompt in a mod | A2 | The visible box can be read and answered | As sessions |
| Player, target, pet, group frames; effects; hotbars; chat box; minimap | hide and redraw | today; finished in A4 | Hide tokens and the reads (`fork_ext/addons/visibility.rs:14-29`); the three bundled UIs use them *(per the audit)* | Hiding the raid list, tracker, meter, breath and stacked bars; click areas of hidden pieces (A4, 4 upstream lines) |
| Buff timers, cast bar, quest goals, pet and group commands | data for mod frames | A3 | Effects without time left; no cast event | All of it; no upstream lines needed |
| Ready check, loot roll | prompt in a mod | A4 | nothing | Needs 4 upstream lines (the answer functions are private to `raidplay.rs:213`, `:244`) |
| Character summary, class mastery, factions | own window + key | A5 | Stats and mastery reads | Training points; spending points (with confirm) |
| Start menu; chat line | replace in place | A6, only if wanted | Open and read the visible menu; chat reads and send | The one place the old "divert" lines are needed |
| Friends, guild, who, group search | own window + key | A7 | Open the game's window | Reads and commands |
| Crafting | own window + key | A8 | Open the game's window | Reads and commands; 0 to 1 upstream line |
| Trade, duel, blacksmith | hidden session | A9 | nothing | 3 upstream lines; money and items change hands |
| Mail, auction | own window + key | A10 | Open the game's window | Reads and commands |
| Mercenary, dye picker, fishing | hidden session | A11 | nothing | 2 to 3 upstream lines |
| Name plates, damage numbers, rings | hide and redraw | A12 | nothing | A screen position for a world point; hide switches (up to 3 lines) |
| World map | already the fork's | A13 | Open, markers, minimap view; Round Minimap and Map Search mods | A mod-drawn full map |

**Not replaceable, by the owner's decisions:** login, character select and creation; Options and the Controls
page (mods add their own switches and keys instead); Mods & Addons, its key and its "Use the game's windows"
switch; the report and support forms; the updater; GM panels; the quit prompt. **A limit on looks:** mod windows
cannot use the game's own lettering yet (`eqoa.fonts.game` is unsupported, `SUPPORT-TABLE.md:215`).

## 3. The A1 design for the modern UI

**What is built.** Three bundled mod windows, each on its own key: **Bags** (the Bag Viewer, given a default key),
**Spellbook** and **Journal** (new). Each opens over play: the character can keep walking, and the game's Start
menu with its Items, Abilities and Quests pages is untouched. The key again, Escape, or the window's X closes it.

**What already exists for it.** A mod registers a key with a default (`eqoa.input.bind`); it gets a row on the
Controls page, the player can change it, and it is saved (`fork_ext/bindpage.rs:132`, `:10`). The game always
wins: a default the game, a macro or another mod already has is refused, and the row says why (`:543-548`,
`:578-579`, `:598-603`). A key with Shift, Ctrl or Alt is a key of its own; when a mod holds it the game does not
also act on the plain key (`:632-646`). Mod keys fire only in plain play: not while typing, dead, or with a game
window up (`fork_ext/addons/input.rs:338`, `fork_ext/play.rs:5419-5427`). The manifest's `replaces` list, the
"Replaces game windows" line and the "Use the game's windows" switch are built (`fork_ext/addons/discovery.rs:41`,
`fork_ext/addons/runtime.rs:970`, `:1021`).

**What the contract means in this design.** It is still built in A1, because every later round uses it.

| Part | In the modern design | Status |
|---|---|---|
| Session | One opening of a window, with a handle that actions belong to. In A1: every opening of a mod's Bags, Spellbook or Journal, and every corpse, merchant and bank the Bag Viewer holds. A confirm ("destroy?") and the server's "which slot?" question belong to a session. A2's dialogs are the same thing | not built |
| Pass | "Not mine, show the game's window" for one opening. In A1 it applies to corpse, merchant and bank; the three new windows hide nothing, so there is nothing to pass | not built |
| Focus | While a mod window is open it may take the controller's d-pad, confirm and back (and hold movement if it asks), so a window can be used with a controller once it is open. Ends on close, error, disable | not built |
| Label | Mods & Addons shows two lines per mod before it is enabled: **Provides: Bags (B), Spellbook, Journal** (new, from a `provides` list in the manifest) and **Replaces game windows: Loot, Merchant, Bank** (exists) | half built |
| The switch | "Use the game's windows" gives back every game window a mod hides, at once. The three new windows hide nothing: the game's own pages are always there in the Start menu. Proposed: the switch also closes open mod windows | exists; the proposal is new |

**Upstream lines.** The old design's seams 1 and 2 (one line at the start of `draw_hud`, `play.rs:10244`, and one
before the Start menu and Items draw, `play.rs:10465`) existed to turn "Start menu > Items" into the mod's
window. In the modern design that is not wanted, so **A1 needs no upstream line**: keys come through the mod key
rows, reads and actions exist, and focus uses hooks that are already there (`viewer.rs:5636` for keys,
`viewer.rs:3382` for the controller). The two lines are still needed in two cases: (a) A6, if the Start menu or
its pages are ever replaced; (b) A2, perhaps one line, for dialogs and invitations that open outside a server
packet *(to be checked in A2; packet-opened ones are caught by existing hooks, `play.rs:3666`, `:4330`)*.

**Controller.** All 16 buttons are the game's: ✕ attack, □ talk, △ let the target go, ○ use, d-pad quick menus,
L1 and R1 targets, L2 quick chat, R2 chat history, L3 autorun, R3 chat line, Start menu, Select camera style
(`binds.rs:144-189`, `play.rs:7238-7269`). Mod key rows take keyboard keys only *(no controller column was found
in `fork_ext/bindpage.rs`)*. So in A1 a controller player opens bags through Start menu > Items as today. Giving
them the mod's Bags from "Items" is exactly old seams 1 and 2: recommended to leave out of A1 and decide with A6.

**Destroy item and drop quest.** Today the game asks for both (section 1), and mods have neither action
(`SUPPORT-TABLE.md:55`, `:57`). In A1 `destroy(item)` and `drop_quest(quest)` never act at once: each opens an
"are you sure?" that names the item or quest and starts on No. By default the client draws it. A replacement mod
may draw it in its own style, but the client accepts a Yes only as the player's own key press or click on that
question; a mod cannot answer for the player, skip it, or remember the answer. The question ends unanswered when
the window closes, on zoning, or after a short time. It can be the client's own box with no upstream line; the
game's Yes/No box cannot be reused without one, because it drops answers it does not know (`play.rs:1182`).

### Keys in use today
Keyboard (`binds.rs:136-217`); the same key in upper case counts as the same key:

| Keys | Game action | Keys | Game action |
|---|---|---|---|
| W A S D | walk | **I J K L** | camera up, left, down, right |
| G (Space where there is no jump) | attack, loot | Space, V | jump |
| Q | talk (hold: details) | E, arrows | quick menu: use, move |
| Tab; `` ` `` | next target; cycle self, group, pet | 1 to 6; F | target self, group 2 to 5, pet; assist |
| Enter, /, T, R | chat line, command, tell, reply | C (hold); Z | chat history; quick chat |
| Escape; Backspace | Start menu; back | X; P | autorun; camera style |
| M, = and -, O | map, zoom, group waypoint | N; U; H | minimap; quest helper; hide UI |
| Home, End, PgUp, PgDn | orbit and tilt camera | F1; F5 to F8; F9 | Controls; group commands; art swap |

Kept by the client, never a mod's: F3, F11, F12, Escape, Alt+Enter, Ctrl+C/V/X (`binds.rs:276-278`,
`fork_ext/bindpage.rs:419-426`). Fork and bundled mods: F10 Mods & Addons; **B Stats Box**; F2 Map Search, F4
clear map guide (`bundled-mods/*/main.lua`). The player may also put any key on the bar, item and weapon-set
slot rows (empty by default, `binds.rs:152-180`); such a key is then the game's.

**Free keys:** Y, 7, 8, 9, 0, Insert, Delete; punctuation such as `[ ] ; ' , .` *(not verified as key names)*; and
any Shift, Ctrl or Alt combination. **The owner's examples are not free:** K is camera down and J is camera left
(the game's), and B is the bundled Stats Box's default.

### Proposed defaults
| Window | Default | Note |
|---|---|---|
| Bags (A1) | **B** | Free in the game. The Stats Box moves to **Y**. A player who put the Stats Box on B themselves keeps it; Bags then has no key and its row says why |
| Spellbook (A1) | **Shift+K** | K alone is the game's camera key and would do nothing for a mod |
| Journal (A1) | **Shift+J** | Same reason |
| Later: Character (A5), Friends and Guild (A7), Crafting (A8) | Shift+C, Shift+F, Shift+G, Shift+R | The plain letters are the game's (chat history, assist, attack, reply) |

**If the owner wants plain K and J:** the game's camera rows must leave those keys first. The player can do that
on the Controls page (once per computer); K and J are then free for the mods. Changing the default layout for
everyone means editing upstream's key table and the vanilla controls: not recommended. Whether the game uses a
held Shift for anything else in play was not searched *(not verified)*.

## 4. What the owner approves

A1, in the modern design:

1. Three windows on keys: **B** Bags, **Shift+K** Spellbook, **Shift+J** Journal. Press again or Escape to close;
   you can keep walking. The Start menu and its Items, Abilities and Quests pages stay exactly as they are.
2. Each key has a row on the Controls page and can be changed. A game key always wins: a mod key that is taken
   shows an empty row that says why. The Stats Box moves from B to Y.
3. Mods & Addons shows, before you enable a mod, **Provides: ...** (its windows and keys) and **Replaces game
   windows: ...** (what it hides).
4. "Use the game's windows", or disabling the mod, at once: corpse, merchant and bank are the game's windows
   again and open mod windows close.
5. Destroy item and drop quest work from a mod window and always ask "are you sure?" first, starting on No. With
   a controller, bags still open from Start menu > Items; no controller button opens a mod window yet.

**A2 (NPC dialogs):** unchanged by the modern design. A dialog is opened by the server, not by a key, so the mod's
dialog window appears in place of the game's when an NPC speaks, and a dialog the mod does not handle is passed
back to the game's window. It reuses A1's session, pass and focus, and may need one upstream line.
