# Replacing the game's windows in eqoa/1 (design)

Design only, written 2026-11-18. No game code was changed and nothing was compiled. It extends round 6.5's
window takeover (corpse, merchant, bank) to the rest of the game's windows, for two kinds of mod:

- **Replacement mods** (ElvUI style): the mod draws its own bags, spellbook, dialogs, frames; the game's
  window stays away.
- **Helper mods**: auto-sell, restock, auto-bank, quest turn-in. The mod does a job and the player's own
  windows stay as they are.

**Proposed upstream footprint: 10 inserted one-line hook calls, firm, across 4 files; at most 22 if every
optional item is built.** Round 6.5's 4 lines are already in and are not counted again. No upstream line is
deleted or rewritten. An earlier draft of this document proposed 126 lines; "Why not drive the hidden
windows" below says why this design needs far fewer.

**Baseline.** Every `file:line` below was read on `feat/eqoa-api-r6.5` at `20b3eebb` (not merged; the audit
branch is older). Paths are relative to `crates/eqoa/src/`. Reproduce with `git show 20b3eebb:<path>`.
Statements marked *check in the round* were not verified and must be before that round builds on them.

Sources: [plan and owner decisions](../EQOA-MOD-API-PLAN.md) (2026-11-17: enabling a mod is the permission),
[helper ideas](../../backlog.md), round 6.5's report, seams note and spec on `feat/eqoa-api-r6.5`, [FORK.md](../../../FORK.md).

## What the code shows

Three facts decide the design.

1. **A window only shows and asks; `Play` does.** Each native window returns a small list of requests and
   `Play` carries them out: the spellbook's `Out` (`spellbook.rs:80`) in `book_actions` (`play.rs:7095`), the
   item menu's `ItemOut` (`itemmenu.rs:42`), the journal's `QuestUp` (`play.rs:7579`), the trainer's `TrainUp`
   (`trainer.rs:136`, `play.rs:7466`), crafting's `CraftUp` (`play.rs:7545`). The game data (inventory,
   abilities, quests, stats, recipes) lives in `Play` and its parts, outside the windows. Round 6's typed
   actions already use this path without any window (`fork_ext/play.rs:2845` drops a quest that way).
2. **Opening a window is a mode number plus a flag, acted on at the next HUD frame.** For example the
   spellbook: `self.mode = 5; self.book_open = true` (`play.rs:7145`), opened and drawn in the window pass of
   `draw_hud` (`play.rs:9848`). The whole pass is one stretch of one function (`play.rs:9806..9862`).
3. **The fork already runs at the right moments.** `fork_ext/play.rs` is a child module of `play.rs`
   (`play.rs:45`), so it reads and calls `Play`'s private fields and functions. Existing hook calls wrap every
   server packet (`play.rs:3367`, `:3964`), start every game step (`play.rs:3325`), see the merged pad report
   before the game does (`viewer.rs:3104`), and see keyboard and mouse events before native dispatch
   (`viewer.rs:5209`).

So a replacement mod does not need the native window at all. It needs (a) the open sent to it instead of to
the native window, (b) reads of the same data, (c) typed actions that call what the window's buttons call.
Only (a) needs new upstream lines, and one line covers every window that opens through a mode.

### Why not drive the hidden windows

The earlier draft kept each native window running unseen and pressed its keys for the mod, which needs a
reader, a draw hook and a key hook in every window file (126 lines) and breaks whenever upstream changes a
window's layout or key handling. Round 6 already met the limit of that approach: it refuses row actions
wherever a window's page or popup is private (`fork_ext/play.rs:2805`, backlog "Row/dialog mod actions").
Typed actions avoid the problem, so the five accessors proposed in `round-6b-seams.md` are not needed for
replacement either.

## The contract: one model for every window

**Claim, session, read, act, close.** All additive under `eqoa/1`.

| Step | Call | Rule |
|---|---|---|
| Claim | `eqoa.windows.takeover(kind, {scope})`, `release(kind?)`, `taken(kind)` | Unchanged from 6.5. One holder per kind; a second mod gets `busy` naming the holder. A window already open stays native until it closes. |
| Session | event `windows.session` `{phase="opened"\|"changed"\|"closed", session}`; `eqoa.windows.session(kind)` | When a claimed window would open, the native one does not show and the holder gets a session instead: `{ref, kind, origin="player"\|"mod"\|"server", opened_from="start_menu"\|"hud"\|"key"\|"npc"?, target:EntityRef?, revision, data}`. `data` is the kind's own table (dialog text and options, craft tray, ...). A mod that subscribes late reads the current session with `session(kind)`. |
| Read | the existing modules (`inventory`, `abilities`, `quests`, `mastery`, `group`, `chat`, ...) | The session carries only what belongs to this one opening. Everything else is read as today. |
| Act | `eqoa.actions.*` | Typed actions, one native operation each, same results and refusals as today (`sent` is not `confirmed`; an uncertain buy, combine, train or turn-in is never replayed). Session-bound actions take `session.ref` and refuse `stale_handle` on an old one. |
| Close | `eqoa.actions.close_window(session.ref)` | Does what the native Back or Close does towards the server (a dialog sends "no choice"; a local window sends nothing). `opened_from` tells the mod where Back leads; `open_window("start_menu")` goes there. The claim stays for the next opening. |
| Hand back | `eqoa.windows.pass(session.ref)` (new) | "Not mine": the game's own window opens for this session exactly as if no mod held the kind. This is also what the host does by itself when the holder errors, is disabled or reloads, so no dialog or prompt is ever lost. |

**Three classes of window, same calls.**

- **Views** (bags, spellbook, journal, character summary, mastery, crafting, Start menu, map): nothing on the
  server is open. Only `scope="all"` means anything (the mod replaces the window); `scope="own"` is refused
  with `invalid_argument`, because a helper never needs the window, only the actions.
- **Server sessions** (corpse, merchant, bank as built; NPC dialogs, invitations, later trade and others):
  6.5's rules stay. `own` hides only what the mod itself opened and can prove is its own; `all` hides every
  opening. Walk-away, zoning and idle time end a hidden session as in 6.5.
- **Always-on HUD pieces** (player, target, group, pet, effects, bars, chat box, minimap, tracker, meter):
  they never "open", so they stay under the existing `eqoa.hud.hide/show` tokens (already `eqoa/1`, used by
  the bundled UIs) rather than gaining a second way to do the same thing.

**Input.** A replaced window leaves the game in play mode (as 6.5 does): the character can still move. A mod
window that wants the pad asks for it: `eqoa.windows.focus(session.ref, {movement=false})` (new). While
held, the menu buttons (d-pad, confirm, back) arrive as `windows.key` events and do not reach the game, and
with `movement=false` the sticks are held too. It is built on the two existing input hooks, releases on
close, error, disable and focus loss, and never applies while a native dialog, the report form, the updater
or Mods & Addons is up.

**Declaring and labelling.** `mod.json` keeps `"api": "eqoa/1"` and its `capabilities` list, and gains an
optional list `"replaces": ["inventory", "spellbook", "hud.target"]`.

- `takeover(kind, {scope="all"})` needs the kind in `replaces` (`not_declared` otherwise) and the existing
  `eqoa.windows.takeover` capability. This applies to corpse, merchant and bank too: no shipped mod uses
  `all` today (the Bag Viewer uses `own` only), so nothing breaks.
- Mods & Addons shows, before the player enables the mod: **Replaces game windows: Bags, Spellbook, Target
  frame**, next to the existing **Actions: can act on its own (...)** line. Both are derived from the
  manifest, never from what the mod says about itself.
- A helper (scope `own`) declares no `replaces` and gets no replacement label.
- Mods & Addons, its key and a **Use the game's windows** switch in it are always native. The switch releases
  every claim at once.

**Versioning.** New kinds, the session event, `session`, `pass`, `focus` and `replaces` are additions and can
land after the freeze. Each kind is a capability path (`eqoa.windows.takeover.inventory`), so a mod asks
`eqoa.capabilities.has(...)` and leaves an unsupported window to the game. Three rules are behaviour and
should be settled **before** round 7 freezes `eqoa/1`: (1) a real window this build cannot take over yet
answers `unsupported`, a string that is no window answers `invalid_argument`; (2) `all` requires `replaces`;
(3) `own` is refused on views. Changing a session's shape, a scope's meaning or an action's outcome later
needs `eqoa/2`.

## The seams (all inserted lines)

Ten firm lines. Each is one inserted call; nothing upstream is moved, deleted or rewritten. The work behind
each call lives in `fork_ext/`.

| # | File and place | Inserted line (names proposed) | What it does |
|---:|---|---|---|
| 1 | before `play.rs:9600`, as the first line of `draw_hud` | `fork_hooks::divert_windows(self);` | Before anything is drawn: if the mode, map or chat line that just opened belongs to a claimed kind, undo the open (mode 0, open flag cleared), start the session. Covers every mode window. Openings made by a server packet are already caught one step earlier by the existing `end_game_event` call. |
| 2 | before `play.rs:9862` | `fork_hooks::divert_items(self);` | The bags are the one window that opens and draws inside the same pass (Start menu's Items entry, `play.rs:7068..7070`), so seam 1 alone would let them flash for a frame. |
| 3 | before `hudlayout.rs:404`, in `end` after the shift is restored | `if crate::fork_ext::piece_hidden(ui, m.piece, m.from) { return; }` | Drops the quads of a hidden piece. One line for all eleven pieces (`hudlayout.rs:36`), including the four that cannot be hidden today. |
| 4 | `hud.rs:388`, first line of `note` | `if fork_hud::skip_pick(what) { return; }` | A hidden piece registers no click area. Today the player frame's is registered whatever is hidden (`hud.rs:416`), and the raid list is drawn past the group check (`hud.rs:448` against `:534`). |
| 5 | `hud.rs:371`, first line of `raid_wheel` | `if fork_hud::raid_hidden() { return false; }` | No wheel capture over a hidden raid list. |
| 6 | `play.rs:10191`, first line of `meter_wheel` | `if fork_hooks::meter_hidden() { return false; }` | The same for the DPS meter. |
| 7 | `raidplay.rs`, module scope | `#[path = "fork_ext/win_raid.rs"] mod fork_win;` | Child module: reaches the private `make_ready` and `make_roll` (`raidplay.rs:213`, `:244`). |
| 8 | before `raidplay.rs:551`, in `run_rolls` after the two `expire` calls | `if fork_win::strips_taken(self) { return; }` | Ready-check and loot-roll strips not drawn; their timers still run (`:548..549`). They are not HUD pieces, so seam 3 does not reach them. |
| 9 | `raidplay.rs:454`, first line of `roll_key` | `if fork_win::strips_taken(self) { return false; }` | Hidden strips take no keys. |
| 10 | `raidplay.rs:519`, first line of `roll_point` | `if fork_win::strips_taken(self) { return false; }` | Hidden strips take no clicks. |

Optional, each decided in its own round and listed for the owner like 6.5's:

| Item | Lines | Why it may be needed |
|---|---:|---|
| Class mastery inspected skill | 1 | The reply to "inspect" is dropped unless the native window is in mode 40 (`play.rs:7363`); buy and refund need it. The fork may be able to keep its own copy instead. |
| Target detail box | 1 | The box at `play.rs:9892` is not a HUD piece. Only if a target-frame mod wants it gone. |
| World map terrain for a mod-drawn map | 1 | Only if the existing map child (`map/mod.rs:90`) cannot render a full-size layer. |
| The long tail (trade, repair, mail, auction, duel, fishing, group search, links, combat record) | up to 9 | At most one child-module line per file (`npcw.rs`, `support.rs`, `mail.rs`, `bankplay.rs`, `duelplay.rs`, `fishplay.rs`, `liveplay.rs`, `linkplay.rs`, `recordplay.rs`) where the action function is private to that file, as in seam 7. |

Zero-line parts, on hooks that exist: pad focus (`viewer.rs:3104` hands the fork the pad report before
`console.poll`), keyboard and mouse (`viewer.rs:5209`), packet-opened windows (`play.rs:3367`, `:3964`),
per-step upkeep (`play.rs:3325`), chat box and bars and effects and minimap hiding (the existing
`fork_hud::` calls in `hud.rs` and `play.rs`).

## Window by window

"Seams" names the numbered rows of the table above. "Today" is what `eqoa/1` has on the baseline.

### Bags and worn equipment (value: high; the Bag Viewer exists)

- **Game:** a child of the Start menu, mode 13. Menus rebuilt each frame from the inventory (`play.rs:7022`);
  opened by the Start menu's Items entry (`play.rs:7068`); requests are `ItemOut` (`itemmenu.rs:42`): use or
  equip, swap, quick slot, gear set, link item, hide look, destroy. Equip logic `inventory.rs:206`.
- **Today:** `inventory.get`, `items.*`, item refs; `use_item`, `equip`, `swap_items`, `quick_slot`,
  `weapon_set`, `prepare_weapon_set`; `open_window("inventory")`.
- **Missing:** the diversion; actions for gear-set save or rename, link item to the chat line, hide/show
  look (all three are `ItemOut` values `Play` already carries out). Destroy stays unsupported by every route.
- **Seams:** 1 and 2. **Risks:** the game's "waiting for the server" lock after an equip (`self.waiting`,
  `play.rs:7071`) must refuse a second action; item order changes after a swap, so actions use item refs.

### Spellbook and abilities (value: high)

- **Game:** mode 5, flag `book_open` (`play.rs:7145`), requests `spellbook::Out` (`spellbook.rs:80`): memorize
  to a slot, arrange, unmemorize all; carried out in `book_actions` (`play.rs:7095`).
- **Today:** `abilities.*`, `hotbar.list`, `cooldowns.get`; `cast`, `use_hotbar`, `assign_ability`,
  `unmemorize_ability`, `unmemorize_all`, `arrange_ability`. Complete for a replacement.
- **Missing:** the diversion only. **Seams:** 1. **Risks:** the slot question the server may ask after a
  memorize (`slot_asked`, `play.rs:7104`) must reach the mod as a session change, not vanish.

### Quest journal (value: medium)

- **Game:** mode 22, flag `quest_open`; data is up to eight titles and a server stamp (`questlog.rs:17`); the
  only request is Drop (`play.rs:7590`). The client holds no objective progress, so a mod cannot show any.
- **Today:** `quests.list/get`, quest events, `drop_quest`, `open_window("quests")`. Complete.
- **Missing:** the diversion only. **Seams:** 1.

### NPC dialogs: hail, quest offers, turn-ins (value: highest for helpers)

- **Game:** two windows. Plain text (mode 9): shown at once in play, else queued, eleven at most
  (`play.rs:865..872`). Text with answers (mode 7): shown at once in play, else one waits, a further one is
  refused to the server (`play.rs:928..943`). An answer sends the dialog's id and the choice (`play.rs:1013`).
  Waiting ones open when play resumes (`play.rs:4029..4037`). On the wire a dialog is an id, text and options
  and **no speaker** (`crates/net/src/session.rs:1107`, `:1109`).
- **Today:** `talk`, `interact`; the visible dialog through `windows.get`; `dialog_answer` only on the
  focused native window.
- **Missing:** dialog sessions (kinds `text_dialog`, `response_dialog`) with text, options and the target
  the player or mod last talked to; `dialog_answer(session.ref, option)`; `pass`. The fork keeps diverted
  dialogs in its own queue, one session at a time. To answer, it puts the dialog back in the native window
  for that one call and presses the game's own keys, so the send is the game's.
- **Scope:** `all` only at first. Because the wire names no speaker, the client cannot prove a dialog
  answers the mod's own `talk`. A turn-in helper therefore holds `all`, answers the dialogs it recognises by
  their current text, and calls `pass` on every other one, which the player then sees as a normal game dialog.
- **Seams:** none new (the existing packet hooks, plus seam 1 for dialogs that open from the queue).
- **Risks:** the largest of the design. A hidden dialog does not hold the player in place, so the fork must
  close it (answer "none") on walk-away and zoning as 6.5 does; what the live server does with a late or
  missing answer is unknown. With the game's own queue bypassed, the fork's queue must keep the game's
  limits (one answerable dialog waiting, the next refused). A wrong match answers the wrong quest: the mod
  matches on the dialog's own text, never on "option 1". An answer's result is `sent`, never `confirmed`.

### Invitations and offers (value: medium)

- **Game:** yes/no prompts in mode 14. Group invitation (`play.rs:6710`; answered by `accept_invite` and
  `decline_invite`, `play.rs:6765`, `:6779`); summons or resurrection (`play.rs:6723`; answers at
  `play.rs:985`). The game asks again each step while an invitation waits (`play.rs:4038`).
- **Missing:** kinds `group_invite` and `revive_offer`, and `actions.answer(session.ref, yes)`. The fork
  parks a diverted invitation outside the game's list so the game does not ask again, and puts it back for
  the answer. Quit, report, guild and GM prompts are never claimable.
- **Seams:** 1. **Risks:** an invitation withdrawn by the server while parked becomes a refused answer.

### Trainer and character summary (value: medium)

- **Game:** mode 12 is the character summary, opened from the Start menu or the HUD (`play.rs:7185`). Spending
  points is one request, `TrainUp::Train` to `trainer::train_stats` (`trainer.rs:667`, `play.rs:7476`). Points
  and the stat cap arrive in packets (`play.rs:881`, `:906`, `:3602`). Talking to a trainer NPC sends its own
  opcode (`play.rs:1050`); nothing on the baseline opens mode 12 from the server, so what a trainer says
  arrives as the dialogs above.
- **Today:** `player.stats/get`, `open_window("summary")`.
- **Missing:** `actions.train_stats({...})` (the seven counts, checked against points and cap); training
  points and cap in `player.get` (*check in the round* whether they are there). **Seams:** 1.

### Class mastery (value: medium)

- **Game:** mode 40; part `classmastery.rs:163`; replies handled in `run_class_mastery` (`play.rs:7346`).
- **Today:** `mastery.get/skills/selected`; `mastery_share`, `mastery_browse`, `mastery_buy`,
  `mastery_refund`, the last three only while the native window is open (`fork_ext/play.rs:2874`).
- **Missing:** the same actions on a taken session. **Seams:** 1, plus the optional one (inspected skill).

### Crafting (value: medium; on the owner's helper list)

- **Game:** recipe book (mode 6) and experiment (mode 37). The part is `craft.rs:208` with public state
  (skills, a copy of the inventory, the combine tray, recipes, pending request) and public requests
  (`craft.rs:288..360`, `send_combine` `:322`, `add_to_combine` `:618`, `remove_from_combine` `:637`). Opening
  is a request in a list `Play` owns (`play.rs:7512..7521`); replies `craft.rs:394`.
- **Today:** only `open_window("recipes"|"experiment")`.
- **Missing:** everything else: a `craft` read (skills, recipes, detail, tray, pending) and typed actions
  (add, remove, combine, make or remove a recipe, answer the server's question). *Check in the round* how the
  experiment's question is answered (`craftwnd.rs:714`). **Seams:** 1.
- **Risks:** the tray works on a copy of the inventory taken at open; a timeout must never send a second
  combine; the server's token stays private.

### Group, raid, ready checks and loot rolls (value: medium; high for raiders)

- **Game:** group bars and the raid list are one HUD piece (`hud.rs:434..448`). Group commands go through
  `group_action` (`play.rs:8139`), raid commands through `raid_action` (`raidplay.rs:271`). Ready-check and
  roll strips are drawn in play, over nothing (`raidplay.rs:546`). Group search is mode 42 (`liveplay.rs:234`).
- **Today:** `group.get` with members, invitations and raid rows (`fork_ext/api_capture.rs:204`); `target`,
  `assist`; `hud.hide("group_panel")`, which misses the raid list.
- **Missing:** typed group and raid commands (*check in the round* which `group_action` takes); kinds
  `raid_ready` and `raid_roll` with their choices and deadline; hiding the raid list.
- **Seams:** 3, 4, 5 (frames) and 7 to 10 (strips). **Risks:** roll and ready deadlines keep running hidden;
  a missed one is a lost roll, so the host passes the strip back to the game when its holder is gone.

### Chat (value: high; the bundled UIs already replace the box)

- **Game:** the chat box is a HUD piece drawn through an existing fork hook (`play.rs:9798`); the line is the
  edit box, mode 2 (`play.rs:7691`); history is mode 33.
- **Today:** `chat.history`, the `chat.message` event, `get_channel`, `actions.chat/command`,
  `hud.hide("chat")` (which also stops its click, `play.rs:5470`), `input.capture_text`.
- **Missing:** kind `chat_entry` (the game's chat key opens the mod's box); the player's tab setup
  (`play.rs:9792`) and the item links in a line (`play.rs:9793`) as reads. **Seams:** 1.
- **Risks:** tells must not be misrouted by a tab; password and report text never enter a session.

### Target and HUD pieces (value: high; partly done)

- **Game:** eleven movable pieces (`hudlayout.rs:36`).
- **Today:** hide tokens for health, power, both experience bars, compass, target, group panel, group
  markers, pet, effects, hotbar, chat, minimap (`fork_ext/addons/visibility.rs:14`); `hud.layout`, `hud.frame`,
  pointer claims; the reads to draw replacements.
- **Missing:** hide for the raid list, quest tracker, DPS meter and breath gauge; click areas of hidden
  pieces. **Seams:** 3 to 6. *Check in the round* that every piece's text lands in the same quad list seam 3
  trims. **Risks:** Move HUD, Hide UI and the report button must keep working with pieces hidden.

### Start menu (value: medium) and options (not proposed)

- **Start menu:** mode 13 (`play.rs:6835`); the entry list is built at open with the server's feature and GM
  checks (`play.rs:7042..7050`); an entry runs through `start_actions` (`play.rs:7137`). Today:
  `open_window("start_menu")` and the visible rows. Missing: the entry list as session data without the
  native menu visible, and `menu_command(session.ref, command)`. **Seams:** 1.
- **Options (mode 34): keep native.** It holds key rebinding with physical capture, save or discard, sound
  preview and host settings (`options.rs:433`). Replacing it has the highest risk and the least value; a mod
  adds its own switches through `eqoa.options.*` as today. Revisit only on request.

### World map and minimap (value: low; both are already the fork's own)

- **Game:** `map/mod.rs:378` show, `:393` close, `:445` keys (every key while up, `play.rs:6473`).
- **Today:** `map.get/open/markers`, the minimap `map.view` texture, `hud.hide("minimap")`.
- **Missing:** kind `map` and a full-size terrain layer for a mod-drawn map. **Seams:** 1, plus the optional
  one. Last, because the native map is itself fork work that mods can already extend.

### The long tail

Trade (modes 15, 16), repair (19), friends, ignore and guild (17, 18, 20, 21, 26), mail (23, 24, 28, 29), who
(35), factions (41), combat record (60), auction (39), group search (42), fishing (36), duel (43, 44), item
link windows. Line 1 already diverts each of them; what each lacks is its read and its typed actions. Build
one only when a mod asks for it. **Never claimable:** login and password, reports and support (31, 32), the
updater, Mods & Addons, the loading screen, GM panels (4, 27), the quit prompt.

## Order of small rounds

Each round: the owner approves the five-line "what you'll see" first; one example mod or option proves it.

| Round | Content | Upstream lines | The player can check |
|---|---|---:|---|
| W0 | Helper mods on 6.5 as built: auto-sell junk, restock to N, auto-bank marked items. No API change. Needs 6.5's owner test passed. | 0 | Each helper works; opening the merchant by hand still shows the game's window. |
| W1 | The contract (session, `pass`, `focus`, `replaces`, label, the "use the game's windows" switch) and the first three views: bags, spellbook, journal. Bag Viewer option "open on the game's bag key". | 2 | The bag key opens the mod's bags; disabling the mod gives the game's bags back at once. |
| W2 | NPC dialogs, invitations, revive offers. Example: a small dialog window and a turn-in helper. | 0 | Hail an NPC, read and answer in the mod's window; an unknown dialog shows as the game's. |
| W3 | Character summary with training; class mastery. | 0 to 1 | Spend one point and buy one skill from a mod window. |
| W4 | Crafting reads and actions. | 0 | Browse recipes and do one combine from a mod window. |
| W5 | HUD completion (all eleven pieces and their clicks), raid ready and roll strips. | 8 (+1) | Hide the target frame and click where it was: nothing happens. Answer a ready check in a mod. |
| W6 | Start menu, chat entry, chat tabs and links. | 0 | The Start key opens a mod menu; Enter types into a mod chat box. |
| W7 | World map session and terrain layer. | 0 to 1 | A mod-drawn map opens on the map key. |
| W8 | Long tail, one window per request. | up to 9 | Per window. |

Firm total 10 (W1: 2, W5: 8); ceiling 22. W1 comes first because everything else rests on its contract; W2
second because quest turn-ins are the helper the owner asked for and it costs no upstream line.

## Risks

1. **Input and getting stuck.** A replaced window no longer pauses the game's controls. If a mod takes the
   pad and then stops drawing, the player could be locked out. Answers: focus ends on error, disable and
   focus loss; every session passes back to the game's window when its holder is gone; Mods & Addons and the
   "use the game's windows" switch are always native.
2. **Hidden server conversations.** Dialogs carry no speaker, so "is this dialog mine?" cannot be proven;
   hidden dialogs bypass the game's own queue; the server's reaction to late or missing answers is known
   only from live play. A wrong answer can accept or hand in the wrong quest. `all` plus `pass`, text-based
   matching, close on walk-away and never replaying an answer limit this; only the owner's live test settles it.
3. **Drift from upstream.** The mod's actions call the game's functions directly, not through its windows.
   When upstream changes what a button does, the typed action can fall behind without a compile error. Each
   window gets a test that presses the native button and calls the mod action on the same state and compares
   what was sent; the sync review reads the window files' diffs.
4. **Sounds.** The game queues the open sound when it sets the mode (`play.rs:957`), before the fork diverts,
   so a replaced window still plays its open cue, as in 6.5. Removing it costs a line per sound; not proposed.
5. **One holder per window.** Two mods that both want the NPC dialog (a UI skin and a turn-in helper) cannot
   run together; the second is told `busy` by name. Chaining holders is possible later, not in this design.
6. **State a window owns.** A few values live only in a native window (points staged in the trainer, the
   held spell while arranging). A replacement keeps its own; nothing is shared with a half-open native window
   because a claimed window never opens. `pass` therefore always opens the native window fresh.

## Testing per round

Negative control for each hook (remove the line, the test fails); zero-addon path does no work (as 6.5's
idle fast path); second-holder conflict; holder error, reload and disable with a session open; a native
window already open when the claim is made; zoning with a session open; stale session ref refused; native
button against mod action on the same state. Footprint check against the reviewed base, wasm check, then the
owner's live test from a branch build, because server order, range and sound can only be seen live.

## Owner questions

1. **Order:** helpers first (W0), then bags, spellbook and journal (W1), then NPC dialogs (W2). Agreed?
2. **NPC dialogs:** enabling a mod already lets it act on its own. Should answering NPC dialogs (accepting
   and handing in quests) get its own line on the label, "Can answer NPC dialogs"? Proposed: yes.
3. **Bag key:** when a replacement mod is on, the game's own key opens the mod's window. For the bundled Bag
   Viewer, an option that is off by default. Agreed?
4. **Never replaceable:** options, login, reports, updater, Mods & Addons, GM panels. Agreed?
5. **Timing:** settle the three contract rules before the `eqoa/1` freeze (round 7), build W1 onward after
   it. Or W1 before the freeze?

## Owner answers (2026-11-18)

All five: the recommended option. (1) Order W0-W8 as proposed. (2) "Can answer NPC dialogs" is its own label line.
(3) The game's own key opens a replacement mod's window when that mod is on; for the Bag Viewer an option, off by
default. (4) Options, login, reports, updater, Mods & Addons and GM panels are never replaceable. (5) Settle the three
contract rules before the `eqoa/1` freeze; build W1 onward after it.
