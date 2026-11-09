# Direction review: getting to "upstream plus mods"

**Date:** 2026-11-09 · **By:** Claude Fable (read-only review; nothing built, no code, plan or other doc changed)
**Compared:** `develop` 78e5221 against upstream at the last sync (`91f9e72`) and upstream today (`b4c430f`, 25 commits newer).

Words used here:

- **Upstream**: Winfro's vanilla client, which this fork follows.
- **Sync**: bringing upstream's new work into the fork.
- **Conflict**: a place where upstream and the fork both changed the same lines, so someone has to merge them by hand.
- **Hook** (or seam): one line in upstream's file that calls out to the fork's own file. Cheap to keep.
- **Inline change**: fork code written directly inside upstream's file. Expensive to keep, because it conflicts whenever upstream edits nearby.
- **API**: the set of things a mod is allowed to ask the game for or tell it to do.
- **PR** (pull request): a change offered to upstream for Winfro to accept.

---

## Top 5 changes

1. **Stop rearranging upstream's Options pages.** The fork rewrites upstream's menu layout: it removes upstream's Map page rows, moves upstream's Text, HUD and Raid List rows onto a fork page, and keeps its own list for the UI page. That is why `options.rs` conflicted in about 7 of the last 8 syncs, and why upstream's new Pet Effects row conflicts today. Leave upstream's pages exactly as they are and put every fork row on fork-only pages after them. *Cost:* one small worker task, about a day. *Saves:* the most frequent conflict in the project, and most of the `settings.rs` test conflicts that come with it. Needs nothing from upstream.

2. **Treat the map as upstream's again, and move the fork's map additions out of upstream's map files.** Since 9 November the map files are upstream's files. The fork's records still call them "fork-owned", so no budget watches them, yet the fork carries about 1,150 changed lines inside them and 3,700 lines of its own files in the same folder. The next sync already conflicts in two of them. First correct the records; then move search, guidance, the journal, the round minimap and the theme out, as mods over upstream's map. *Cost:* records, an hour. The move is the largest job here: one API round plus one or two mod rounds. *Saves:* the biggest block of fork code inside upstream's files, and it is the owner's stated preference.

3. **Turn the four small HUD extras into mods first: Stats Box, Target of Target, Mana Bars, HUD numbers.** Upstream's own rules say these three "stay out" of vanilla, so they'll never be upstream features. They're small, the mod API nearly covers them already, and they account for most of the fork's inline code in `hud.rs`, `play.rs`, `settings.rs` and `binds.rs`. *Cost:* one round (a few small API additions plus four small mods). *Saves:* roughly 400 inline lines across the four files that conflict most, and gives the mod API its first real users instead of example mods.

4. **Offer upstream small fixes first, then three neutral seams.** Five bug fixes the fork already carries are plain improvements to vanilla and need no design discussion. After those, three seams, biggest saving first: option rows declared in one table, HUD parts that can each be switched off, and a game-event observer. *Cost:* each PR is a small worker task plus the owner sending it. *Saves:* each accepted PR removes those lines from every later sync for good.

5. **Reorder the mod API plan around this goal and add a safety gate.** Rounds 3 to 5 follow FrontiersForge's module list (bags, bank, merchant, gems). None of that reduces sync cost or moves a fork feature into a mod. Put the HUD extras and the map first, items after, and automation last. Add two gates before any round reaches Nightly: a speed check with no mods enabled, and a test that starts from real game data. Round 2 shipped a build that ran under one frame a second, and three bugs only live play found. *Cost:* plan text only (provided below). *Saves:* rounds that serve the goal, and no repeat of the round 2 regression.

---

## 1. What the fork changes in upstream's files today

### The short picture

- The fork is 355 files of its own and changes in 32 of upstream's files (not counting the map files, see below).
- The mod runtime is cheap to carry. It is about 46,000 lines, all in the fork's own folder, and touches upstream with roughly a dozen call lines in `play.rs` and about 35 lines in `viewer.rs`.
- The expensive part is older: features that were restored inline before the hook rule existed (Stats Box, Target of Target, Mana Bars, the map additions) and the options layout.
- **A dry run of the next sync (upstream today into `develop`) conflicts in 8 files:** `hud.rs`, `map/chrome.rs`, `map/mini.rs`, `options.rs`, `play.rs`, `settings.rs`, `touch.rs`, `viewer.rs`. It is the same set as almost every earlier sync.

### How often each file has conflicted

Counted from the sync records (28 Oct, 31 Oct, 3 Nov, 7 Nov, two on 8 Nov, 9 Nov) plus the dry run of the next one: 8 merges. The 7 Nov record doesn't list its eight files, so those counts are "about".

| File | Conflicted in | File | Conflicted in |
|---|---|---|---|
| `options.rs` | about 7 of 8 | `binds.rs` | 3 of 8 |
| `play.rs` | about 7 of 8 | `touch.rs` | 3 of 8 |
| `settings.rs` | 6 of 8 | `main.rs` | 3 of 8 |
| `hud.rs` | about 5 of 8 | `render/ui.rs`, `render/lib.rs`, `bindpage.rs` | 2 of 8 each |
| `viewer.rs` | 4 of 8 | map files (since upstream restored the map) | 2 of 2 |
| `net/session.rs` | 3 of 8 | `README.md`, `CLAUDE.md` | every time, resolved automatically |

### Every change, grouped

Groups: **(a)** a neutral hook upstream could accept; **(b)** a fork feature that should become a mod over upstream's feature; **(c)** drop in favour of upstream's; **(d)** must stay fork-only.

| # | Change | File(s) | Group | Sync cost | Next step |
|---|---|---|---|---|---|
| 1 | Options page rearrangement: fork's own UI row list, upstream's rows 41–45 removed and replaced by fork rows, upstream's Text/HUD/Raid List moved to "Map & Quests", Textures moved last, walk tests rewritten (+46/−89) | `options.rs` | (c) drop the rearrangement | Highest: about 7 of 8, and conflicting now | Keep upstream's pages untouched; fork rows only on fork pages added after upstream's. Top 5 #1 |
| 2 | Option and setting hooks (`fork_options::change/activate/value_text/items`, `fork_settings::write/parse`) | `options.rs`, `settings.rs` | (a) | Low once #1 is done | Offer as "option rows in one table" (PR 1) |
| 3 | Changed first-run defaults (NPC display 64, distance 650) and the upstream tests edited to match | `settings.rs`, `options.rs`, `play.rs` tests | (d), or (c) | Medium: every upstream test that names the default | Owner decision 5. If kept, set the defaults from the fork's settings file at first run and leave upstream's constants and tests alone |
| 4 | Target of Target: bar, click, option, key label, tests | `play.rs`, `hud.rs`, `settings.rs`, `binds.rs` | (b) | High (sits in the four worst files) | Mod. Needs: an entity's target id, the Assist action. Top 5 #3 |
| 5 | Mana Bars drawing: gauges under group, target and target-of-target | `hud.rs`, `play.rs`, `settings.rs` | (b) | High | Mod. Needs: other players' power on group and entity records |
| 6 | Mana Bars network reading (server messages 0x7E01/0x7E02) | `net/session.rs`, `play.rs` (1 line) | (a), else (d) | Medium (3 of 8) | Ask Winfro to keep the reader with nothing drawn (it is his server's own message). If not, it stays as a small fork-only block |
| 7 | Stats Box: the box, its click, the B key, "opened from the HUD" flag, option | `statbox.rs`, `hud.rs`, `play.rs`, `settings.rs`, `binds.rs`, `viewer.rs` | (b) | High | Mod. Needs: player stats with effect raises and cuts, Class Mastery bonuses, "open the Summary window" |
| 8 | HUD numbers (Target HP %, XP details, HP/Power numbers) | `fork_ext/hud.rs`, `settings.rs` | (b) | Low (already behind a hook) | Mod; the bundled UIs already draw the same things |
| 9 | HUD part switches: about 14 "is this part shown?" checks at draw and click sites, plus wrapped draws of effects, chat and hotbars | `hud.rs`, `play.rs` | (a) | Medium-high (`hud.rs` about 5 of 8) | Offer as "each HUD part can be switched off" (PR 2) |
| 10 | Mod runtime game events: start of frame, before/after each server event, chat line, hit, cooldown, art change | `play.rs` (about 12 lines) | (a) | Low per line; `play.rs` conflicts for other reasons | Offer as "game-event observer" (PR 3) |
| 11 | Mod runtime in the window loop: overlay drawn on the finished frame, input offered to the overlay first, mouse capture, controls pre-step, shutdown | `viewer.rs` (about 35 lines) | (a) | Medium (4 of 8) | Part of PR 3 (overlay and input pre-filter) |
| 12 | Fork fields inside `Play` and `HudState` (`forge_health`, `trainer_from_hud`, `forge_health_hidden`, power and target-of-target fields) | `play.rs`, `hud.rs` | (c) restructure | `Play` is built on one very long line, so any fork field conflicts whenever upstream adds a field (the sync script now merges this automatically) | Keep fork state in the fork's own files, not in upstream's structs. Goes away with #4, #5, #7 and PR 3 |
| 13 | Map search (F2/F4), guidance, creature journal: inline wiring | `play.rs` (about 90 lines), `viewer.rs` (about 45), `binds.rs`, plus `map/search.rs`, `map/journal.rs` | (b) | High | Mod over upstream's map. Needs the map API in section 2. Journal first |
| 14 | Round minimap, theme, Space Mono font, hover cards | `map/mini.rs` (+313/−44), `map/chrome.rs` (+327/−19), `map/theme.rs`, `render/map.rs`, `render/map.wgsl`, `render/ui.rs`, `render/ui.wgsl`, `ui.rs` | (b); a small part (a) | High: both files conflict in the next sync | Mod (a "map skin"). Possibly ask upstream for a Square/Round minimap option. Owner decision 3 |
| 15 | In-map suggestion form | `map/suggest.rs`, `mapsuggest.rs`, `play.rs`, `viewer.rs` | (c) | Medium | Take upstream's edit-box flow. Revisit as a mod only if players miss it |
| 16 | Group waypoint and GM travel adapters, quest marker/tracker toggles duplicated beside upstream's | `groupmark.rs`, `map/mod.rs`, `map/tool.rs`, `play.rs` | (c) | Medium | Upstream has all of these now: take upstream's code and rows unchanged |
| 17 | Key rows for fork features, and upstream tests edited because the fork took keys G, B, F2 | `binds.rs` (+61/−57), `bindpage.rs` | (b) with their features; renamed "Assist Target" label (c) | 3 of 8 | Rows leave with their features. Mods should register their own keys (API gap, section 2) |
| 18 | Local bug and crash reports (F3, HUD button, crash offer, password scrubbing) | `logfile.rs` (9 lines), `main.rs`, `viewer.rs` (6), `hud.rs` (1), `station/api.rs`, `startmenu.rs` | (d) | Low now | Upstream removed reports on purpose and sends petitions to the server instead. Keep; it is already behind hooks |
| 19 | A refused login shows its reason and is logged | `pregame/station/api.rs` (+65), `renderer.rs` (+3) | (a) bug fix | None yet | Offer upstream (fix bundle) |
| 20 | Live-server client check (the 0x7F53 answer, the "CDSS" signature, the `sha2` dependency) | `net/session.rs`, `s2.rs`, `net/Cargo.toml` | (d) unless Winfro wants it | Low | Owner decision 6. Upstream's rules make what the client tells a server "the user's call" |
| 21 | Animation bounds guards (stop a crash on empty or missing animations) | `vi/csprite.rs`, `vi/hsprite.rs` | (a) bug fix | None yet, but it is in upstream's most protected folder | Offer upstream. A clean branch already exists (`fix-animation-oob-panic`) |
| 22 | "No controller plugged in" is no longer treated as a resting controller | `input.rs` | (a) bug fix | None yet | Offer upstream (fix bundle) |
| 23 | Touch: D-pad kept for quick chat, long-press handling | `touch.rs` | (c), else (a) | 3 of 8, and conflicting now: upstream rewrote touch (+862 lines) | Take upstream's new touch code, then re-test. Offer only what is still missing |
| 24 | Chat history scroll key and "keep the hotbar in chat-history mode" | `play.rs` (3 lines) | (d) small | Low | Keep as hooks. Fold into PR 3 if it is accepted |
| 25 | Fork identity: updater address, update notes file, release page, program icon, version label, NOTICE, README, start scripts, CI workflows | `update.rs`, `updatewnd.rs`, `icon.rs`, `build.rs`, `logfile.rs` (1), `NOTICE`, `.github/` | (d) | Low; one or two lines each, mostly auto-resolved | Keep. Optional later PR: one "who publishes this build" file upstream, so a fork changes one file |
| 26 | Extra libraries (Lua, ImGui, a font library) | `Cargo.toml`, `Cargo.lock` | (d) | 1 of 8 | Keep |
| 27 | Windows test fixes: a texture-pack test path, shader files kept with Unix line endings | `texpack.rs`, `.gitattributes` | (a) bug fix | Low | Offer upstream (fix bundle). Also report the two lighting tests that fail on the owner's NVIDIA card on unmodified upstream |
| 28 | `Pick` list reordered, comment rewording, reformatting of upstream lines | `hud.rs`, `play.rs` (`mini_view`), `binds.rs` | (c) | Pure noise that causes conflicts | Restore upstream's order and formatting. Add to worker rules: never reformat upstream lines |

### Housekeeping the records need

- `fork-footprint.json` lists `map/**`, `tracker.rs`, `groupmark.rs`, `mapsuggest.rs`, `render/map.rs` and `render/map.wgsl` as fork-owned. They are upstream's again. Move them to the shared list with measured budgets, so the check sees them.
- It still carries budgets for files that no longer differ from upstream (`lod.rs`, `hideui.rs`, `client_image.rs`, `esf/mesh.rs`, `render/fans.rs`, `web/main.js`), and several budgets are far above what is used (`viewer.rs` allows 255, uses 132; `session.rs` allows 524, uses 281; `play.rs` allows 759, uses 425). Tighten them, or the check can't notice growth.
- The check currently fails in the main checkout only because of a stray file, `du.exe.stackdump` (a crash dump from a disk-usage tool). It is safe to delete; I left it alone.

---

## 2. The fork's features against upstream's

Upstream's rule file is explicit about what vanilla keeps. The map, minimap, markers, waypoints, group waypoint, quest helper and tracker are in, on by default. "The Stats Box, others' mana bars and Target of Target stay out."

| Feature | Upstream today | Fork today | Best home | What the mod API needs | In the plan? |
|---|---|---|---|---|---|
| World map (M) | Disc-drawn world, camps, coaches, towns, community markers, waypoints, GM travel, master on/off switch | Same code, restyled, plus search and hover cards | Upstream's map; fork look as a **skin mod** | Colours, font and badge art for the native map; or hide parts of the native chrome and draw over it | No |
| Minimap (N) | Square, 112 px, dots, quest marks on the rim | Round, with hover cards and live-match highlights | Upstream's minimap; fork's as an **alternative mod** that hides the native one and draws its own | "Hide the native minimap"; the current map view as a picture a mod can draw round; marker and dot data; mouse and tooltip calls | Partly (pictures and mouse calls in round 4; hiding and the view picture are missing) |
| Map search and guidance (F2/F4) | None | Fork only | **Mod** | Marker and entity data; set and clear a guide point (planned); a text box (exists); mod-registered keys; a way to ship the 1.7 MB creature list (today's limit for a mod-readable file is 1 MB) | Partly |
| Creature journal | None | Fork only | **Mod**, and the best first one | Entity-seen events (exist), the mod's own files (exist), a larger data file allowance | Mostly yes |
| Quest tracker, quest marks | Same code as the fork's, with a Quest Helper master switch | Same, with separate toggles | **Upstream's**, unchanged | Nothing | n/a |
| Group waypoint | Key O, same chat protocol | Same (the fork moved to O in the last sync) | **Upstream's** | Nothing | n/a |
| Map suggestion form | Game edit box | In-map form | **Upstream's** | Nothing | n/a |
| Stats Box | Deliberately out | Inline | **Mod** | Player stats with effect changes; Class Mastery bonuses (round 4); open the Summary window | Partly |
| Mana Bars (others' power) | Deliberately out | Inline, reads the server's message | **Mod**, with the network reader staying in the client | Power on group members and on entities | Not listed |
| Target of Target | Deliberately out (upstream has the Assist key, F) | Inline bar | **Mod** | An entity's target id; the Assist action (round 4) | Partly |
| HUD numbers | None | Fork hook | **Mod** | Nothing new | Yes |
| HUD hides (so UI mods can replace parts) | Only "Hide UI" for everything | Per-part switches wired into upstream's HUD | **Runtime**, over an upstream seam (PR 2) | Already there (`eqoa.hud.hide`) | Yes |
| Chat | 1,000 lines, mouse selection and copy | Chat-history scroll tap for the Wereoxxs UI | **Upstream's**, plus the small tap | Exists | Yes |
| Raid list, pet effects | Upstream's | Upstream's. Known clash: the fork's Target of Target bar draws over a long raid list | Upstream's | The clash disappears when Target of Target is a movable mod window | n/a |
| Bug reports | Petitions to the server on F3 | Local zip saved to the Desktop | **Fork-only** | None; not for mods (the sandbox blocks it on purpose) | n/a |
| Combat parser link | None | Fork sends combat events to the companion program | Fork-only for now | Would need a narrow "send to a local companion" permission; mods have no network by design | No; leave |

**Mod over upstream's feature, or alternative mod?**

- *Restyle* (a mod that changes upstream's feature) only works where upstream's code lets its look be changed from outside. Today it doesn't: the minimap's shape and the map's colours are fixed in upstream's code. That needs either a small upstream seam or the fork keeping a hook in those files.
- *Alternative* (a mod that hides upstream's piece and draws its own) needs nothing from upstream's map code beyond a "hide" switch and data. It is the safer route for the round minimap, search and hover cards.
- Recommendation: alternative mods for the minimap and search; a skin for the big map only if upstream accepts a small theme seam; otherwise leave the big map with upstream's look and let search open as its own window.

**The map API the plan is missing** (the plan has zone data, map pictures and "set a waypoint" only):

1. Marker data: camps, coaches, towns, quest marks, community pins, the player's and the group's waypoints.
2. The current map view as a picture (the client renders it, the mod places it, round or square). This avoids a mod redrawing the map every frame, which is the speed risk the plan itself names.
3. Hide switches for the native minimap and for parts of the map's frame.
4. A guide point a mod owns that can follow a moving creature.
5. Keys a mod can register, shown on the Controls page.
6. Large read-only data shipped with a mod (raise the 1 MB limit for packaged data, or load it through the client).

---

## 3. The upstreaming path

### What will and won't be accepted

Upstream's rules: nothing that changes the game's picture or play without the maintainer's say; every change tested, with a "negative control" (proof that the test fails without the change); never commit game data. So each PR must be useful to vanilla by itself, change nothing a player sees, come with tests, and mention no mods, Lua or Forge. The owner sends each one under their own name.

One practical point: PRs are branches cut from upstream's `master` with only the neutral change. They are never cut from `develop`.

### Order

| Step | PR | Why this position | Saves |
|---|---|---|---|
| 0 | **Fix bundle**, as separate small PRs: animation guards (branch exists); refused login shows its reason; no phantom controller; Windows test path; shader line endings. Plus a note about the two lighting tests failing on NVIDIA | No design to argue about; builds trust; tells us how Winfro likes PRs | 6 shared files leave the footprint |
| 1 | **Option rows in one table** | Biggest recurring conflict | `options.rs` and much of `settings.rs` |
| 2 | **HUD parts can be switched off one by one** | Second-worst file pair | Most of the fork's lines in `hud.rs`, several in `play.rs` |
| 3 | **Game-event observer and overlay layer** | Carries the whole mod runtime | The runtime's lines in `play.rs` and `viewer.rs` |
| 4 | Keep the "others' power" reader with nothing drawn | Small; needs Winfro's yes | Part of `session.rs` |
| 5 | Minimap shape and map theme as options, or a small theme seam | Only if the owner wants it (decision 3) | `map/mini.rs`, `map/chrome.rs`, render files |
| 6 | One "who publishes this build" file | Nice to have | A line or two in six files |

Do Top 5 #1 (stop rearranging the options pages) before PR 1. It needs nothing from upstream, and PR 1 is much smaller once the fork's own layout changes are gone.

### PR descriptions the owner could send

**PR 1: Options rows declared in one place**

> Adding an Extended option today means editing six places in `options.rs` and `settings.rs`: the label list, the page's row list, the "change" case, the "value text" case, the save line and the load line. The browser build then picks its pages by position in a hand-written list, which has to be recounted whenever a page is added.
>
> This PR describes each row once (its id, label, page, how it reads and changes its setting, and whether it is native-only) and builds the pages, the save file and the browser's page list from that. Nothing a player sees changes: same pages, same order, same saved file, byte for byte.
>
> It also sets aside ids 1000 and up for builds other than vanilla, so a downstream build's rows can never collide with a new vanilla row.
>
> Tests: the existing walk-through-every-page tests pass unchanged; a new test proves every row is on exactly one page and that the saved file for every setting is identical to the old code's; a planted duplicate id fails it.

**PR 2: Each HUD part can be switched off by itself**

> Hide UI turns everything off at once. The Map and Raid List options each hide their own piece with their own check, and the click handling repeats those checks separately from the drawing. This PR gives every HUD part (health, power, experience, compass, target, group, pet, effects, chat, hotbar, minimap, tracker) one shared "is this part shown?" question, asked both where the part is drawn and where it takes a click.
>
> In vanilla the answer comes only from the existing options and Hide UI, so the picture is identical. What it fixes is the class of bug where a hidden part still takes a click, and it gives a future "hide just the compass" option a single place to live.
>
> Tests: a frame drawn with everything shown is identical to before; for each part, switching it off removes its quads and its click area and nothing else; the control is the same click landing when the part is shown.

**PR 3: A game-event observer, and a layer drawn over the finished frame**

> The client already has several things that watch the game go by: the chat log switch (`EQOA_CHAT_LOG`), the creature watch (`EQOA_WATCH`), the combat record and the bug note. Each is wired into the play loop by hand.
>
> This PR adds one small observer interface with a handful of calls: start of a game frame, before and after each server event is applied, a chat line as shown, a hit, a cooldown, and art reloaded. It also adds one optional layer drawn over the finished frame that is offered input before the game. The chat log and the creature watch are moved onto it as the first users. With no observer installed the cost is one empty check per event, and no behaviour changes.
>
> It makes scripted tests simpler too (a test can record what happened instead of reading printed lines), and it is the kind of seam a damage meter or a recording tool would need, without any of that living in vanilla.
>
> Tests: with no observer every existing test and screenshot is unchanged; a recording observer sees the exact sequence for a scripted fight (target, hit, cooldown, chat line) at 30 and 165 frames a second; removing any one call fails its test.

---

## 4. The mod API plan (rounds 3 to 5)

### Does it serve the goal?

Partly. It builds a solid mod API, and its rule that every hook must be a neutral seam is right. But its order comes from FrontiersForge's module list, not from the owner's goal:

- **Round 3 (items, bank, loot, merchant, gems)** doesn't reduce sync cost and moves no fork feature into a mod. No bundled UI is blocked on it.
- **Round 4** bundles everything else, including the two things the goal needs most (map data, the small actions) with the largest and riskiest piece (full automation, which needs new seams in controls, dialogs, the bank and merchants, an estimated 40 to 60 more lines in upstream's files).
- **"Map as a mod"** sits after round 4 as something to decide later, though it is the largest sync saving available.
- **"Offer the hooks upstream"** is marked optional. For this goal it is the main line of work.
- **Round 5 freezes `eqoa/1`** before the map mods, the API's largest real users, exist.

### What round 2 shows

1. **The speed regression.** Nightly fell below one frame a second with every mod disabled. The new code copied the whole game state on every frame and every network message whether or not anything was listening (about 10 MB and 80,000 small memory requests per frame). It passed 316 tests, an independent review and a release build. The owner found it by playing. Lesson: "costs nothing when no mod is on" must be a test that runs before Nightly, not a principle.
2. **Three live bugs** (Enter opened the game's chat instead of the mod's; a false error on a normal missing file; overlapping hotbar icons). The icon bug came from converting the game's data to the mod's format. The tests didn't see it because they started from hand-built mod data instead of real game data; the round's own review named this gap and deferred it. Lesson: tests must start from game-shaped data, and bundled UIs need a picture comparison.
3. **Develop publishes to players.** A push to `develop` becomes the Nightly build automatically, so a round's first live test is also its release to Nightly players.
4. **Size.** Round 2 produced a 51 KB report and needed four fix rounds. The rounds are too big to review cheaply. Smaller rounds that each end in one real mod are easier to judge: the owner plays the mod.

### What to change

- **Reorder:** harden and offer upstream → HUD extras as mods → map API and map mods → items → actions and the rest → freeze.
- **Scope each round by a real mod**, not by a module list. A function with no user in a bundled mod waits.
- **Add two gates to every round:** the no-mods speed check, and a test from game-shaped data. Add an owner check of a branch build before the merge to `develop` for any round that touches the per-frame path.
- **Split automation out.** Small player-style actions (target, assist, open a window, set a waypoint) go early because the HUD mods need them. Full automation is its own later round with its own owner approval of the new seams.
- **Freeze later.** Promise `eqoa/1` only after the map mods have shipped on it.
- **Stop:** chasing Forge parity for modules no bundled UI uses; adding to the 140 KB spec before the code (generate the reference from the running client instead, section 5); example mods as a round's acceptance when a real mod is available.

Replacement text is under "Proposed plan text" below.

---

## 5. Making mods easy

### What a mod author has today

| Need | Today | Gap |
|---|---|---|
| Getting started | `docs/modding/GETTING-STARTED.md`: one dense page, accurate | Reads like a contract; no step-by-step first mod with pictures |
| Reference | `EQOA-API-SPEC.md`: 140 KB, written as a build target for agents, and it mixes what exists with what is planned | No reference of what actually works in this build |
| Examples | One (`eqoa_example_hearts`, 42 lines) plus three large Forge-style UIs | No small examples per area (target, group, chat, settings, icons) |
| Reloading | Addon Manager: Rescan, Reload, Enable/Disable | Manual. No reload when a file is saved |
| Errors | Addon Manager shows them; `mods/addons_error.log`; unsupported calls are reported, never faked | Good. Missing: a clear on-screen notice at the moment a mod breaks |
| Feature check | `eqoa.capabilities` | Good |
| Testing | Play the game | No way to run a mod without logging in. This is the biggest gap |
| Packaging | A folder with `mod.json` | No version or "needs client X" field that is enforced; no zip install; no update path for a player |
| Sharing | Copy a folder | No listing; no template to start from |
| Settings and profiles | Per-character profiles, saved settings | Good |

### What to add, most useful first

1. **A way to run a mod without the game** (`eqoa mod-test <folder>`): load the mod against a saved game snapshot, run a few frames, print its errors and save a screenshot. The pieces exist already in the tests (the bundled UIs are drawn headless and their pixels counted). This serves authors and agents alike.
2. **A reference generated by the client** (`eqoa mod-api > API.md`): the client already knows every function and whether it works (`eqoa.capabilities.list`). Generate the reference and an editor hints file from that, so the docs can't drift from the build.
3. **A starter template** (`bundled-mods/_template`, or "New mod" in the Addon Manager) with `mod.json`, a window, a saved setting and comments.
4. **Reload on save**, as an Addon Manager "developer mode" switch.
5. **A cookbook:** ten short mods, each one idea (show target health, play a sound on a tell, a movable window with a setting, and so on).
6. **Package rules:** `version` and "needs API revision N" in `mod.json`, checked with a plain message; install by dropping a zip into `mods`.
7. **A place to share:** a public list (a GitHub repository with one page) is enough to start.

### For the owner, who builds mods by briefing agents

- **Keep mods out of the client's build.** A mod is Lua files in a folder. Mod work should need no Rust build, no build lock and no 5-minute waits. Give the fork's own mods (the HUD extras, the map mods) their own folder or their own repository, with a short agent guide. Cheap agents can then work on mods in parallel, which the one-build-at-a-time rule prevents today.
- **A mod brief template** matching the owner's UI rule: five lines of what you'll see, when it shows, what can be changed in settings, what it must never do. The agent answers with a screenshot from `mod-test` before any live test.
- **A short agent guide** (`docs/modding/AGENT-GUIDE.md`, one page): where mods live, how to run `mod-test`, how to read the error log, the generated reference, and "never edit the client for a mod; if the API lacks something, report it".
- **An API wish list.** When an agent finds a gap it adds one line to a list. That list, not Forge's module list, sets the next API round.

---

## Proposed plan text

Ready to paste into `docs/plans/EQOA-MOD-API-PLAN.md`. It replaces the rounds table from round 3 on, the round 3 to 5 sections, "Map as a mod" and "Optional: offer the hooks upstream". Rounds 1 and 2 stay as written.

```markdown
## Goal update (owner, 2026-11-09)

The fork should become upstream plus mods. Neutral hooks are offered upstream; the fork's own
features (Stats Box, Target of Target, Mana Bars, HUD numbers, map search, journal, round minimap,
theme) move out of upstream's files into mods; mods never go upstream. A round is judged by two
numbers as well as its tests: lines of fork code left in upstream's files, and conflicts in a dry-run
merge of origin/master.

## Gates for every round from here on

1. **No-mods speed gate.** The optimized zero-addon tests (frame, menu frame, entity packet, chat
   line) allocate nothing and build no full view. This runs in CI on every develop push.
2. **Game-shaped tests.** Every new API value has a test that starts from Play/protocol-shaped data,
   not a hand-built mod snapshot. Each bundled UI has one picture-comparison test.
3. **Owner check before develop** for any round that changes the per-frame or per-packet path: the
   owner plays a branch build first. Develop publishes Nightly to players.
4. **Footprint goes down or stays level.** A round that raises a shared budget names what it removes
   later, or it is the upstream-seam round.
5. **Never reformat or reorder upstream's lines.**

| Round | Work | Ends with |
|---|---|---|
| 2.5 Harden and tidy | Gates 1-2 built; options pages back to upstream's layout; footprint records corrected; fix PRs prepared for upstream | Owner live test of round 2 passes; dry-run merge conflicts counted |
| 3 HUD extras as mods | Small API additions; Stats Box, Target of Target, Mana Bars, HUD numbers as bundled mods; inline code removed | The four mods on by default; nothing lost for players |
| 3u Upstream seams | Branches from origin/master for PR 1-3, each with a vanilla use and tests | Owner sends them |
| 4 Map API and map mods | Map data API; journal mod, then search and guidance, then round minimap and skin; fork map code removed from upstream's map files | Upstream's map files unchanged or behind one seam |
| 5 Items | inventory, item, bank, loot, merchant, gems | A bag viewer with a loot log |
| 6 Actions and the rest | Full automation, quests, camera, dialogs | Owner approves the new seams first |
| 7 Finish | Audit, generated reference, support table, freeze eqoa/1 | Owner approves the release |

### Round 2.5: harden and tidy (no new API)

1. Make the zero-addon allocation tests a required CI gate. Add a Play-shaped fixture helper and move
   the three bundled-UI fixtures onto it (round 2 review item L6). Add one picture comparison per
   bundled UI.
2. Options: restore upstream's Extended pages exactly (UI, Map, Textures and the rest: rows, order,
   ids 41-45). Fork rows live only on fork pages placed after upstream's last page. Keep every saved
   key. Upstream's walk tests return to upstream's text; fork pages get their own walk test in
   fork_ext/options.rs. Target: options.rs differs from upstream by hook lines only.
3. Footprint records: list the map family (map/**, tracker.rs, groupmark.rs, mapsuggest.rs,
   render/map.rs, render/map.wgsl) as shared with measured budgets; remove budgets for files that
   no longer differ; tighten every budget to its measured value.
4. Restore upstream's order and formatting where the fork only moved things (the Pick list in hud.rs,
   mini_view in play.rs, the reflowed lines in binds.rs).
5. Prepare the fix PRs from origin/master, one branch each: animation guards; refused login shows
   its reason; no phantom controller; texpack test path on Windows; .gitattributes LF for shaders.
   Write the note about the two AO tests on NVIDIA. The owner sends them.
6. Report the before and after of both numbers: fork lines in upstream files, dry-run conflicts.

### Round 3: the HUD extras become mods

API additions (small; each tested from game-shaped data):
- `eqoa.player.stats()`: each stat's base and current value and the effect change on it; armour and
  resists as the Summary window shows them; Class Mastery bonuses when received.
- `power` on group members and entities when the server sends it (nil otherwise); `target_id` on
  entities.
- `eqoa.actions.assist()` and `eqoa.windows.open("summary")`: player-style actions only, through the
  same path as the key or click.
- `eqoa.input.bind{id, label, default_key}`: a key the mod owns, shown on the Controls page.

Mods (bundled, on by default, each in its own folder, no client build needed to change them):
`eqoa_stats_box`, `eqoa_target_of_target`, `eqoa_mana_bars`, `eqoa_hud_numbers`.

Then remove from upstream's files: the Stats Box, Target of Target and Mana Bars drawing, their
HudState and Play fields, their option rows, settings and key rows. Saved settings migrate into each
mod's settings once. The network reader for others' power stays (offered upstream separately).
Owner approves each mod's five-line description first (UI rule). Acceptance: the owner sees the same
four features; hud.rs, play.rs, settings.rs and binds.rs lose the matching lines.

### Round 3u: offer the seams upstream

One branch from origin/master per PR, no Lua, no mod names, each with a use inside vanilla, tests
with a negative control, and the vanilla test suite passing:
1. Option rows declared in one table; ids 1000+ reserved for other builds.
2. Each HUD part can be switched off by itself (one question at the draw and at the click).
3. Game-event observer and a layer over the finished frame (chat log and creature watch moved onto
   it as first users).
The owner opens each PR under their own name. When one is accepted, the next sync deletes the
matching fork lines and lowers the budget.

### Round 4: map API and map mods

Upstream's map is the base. Add to the API:
- `eqoa.map.markers(world_id)`: camps, coaches, towns, quest marks, community pins, personal and
  group waypoints.
- `eqoa.map.view{center, zoom, size}`: the client renders the current map view to a texture; the mod
  places it, round or square. No per-frame map drawing in Lua.
- `eqoa.hud.hide("minimap")`, and hide switches for parts of the map's frame.
- `eqoa.map.set_guide(point or entity)`: a guide the mod owns that can follow a moving creature.
- Packaged read-only data above 1 MiB (the creature catalogue), read through the client.
Mods, in this order, each with the owner's five-line approval: the creature journal; search and
guidance (F2/F4); the round minimap with hover cards; the map skin (only if a theme seam exists).
Then take upstream's map files unchanged. Anything that can't move behind the API is listed for the
owner with its sync cost. Measure frame time with the map mods on before deciding (gate 1 covers
them off).

### Round 5: items

As the former round 3. Start only when a bundled mod needs it.

### Round 6: actions and the rest

As the former round 4 without the map and without the small actions done in round 3. Full
automation needs new seams in controls, dialogs, bank and merchant code: the lead shows the owner
the exact lines before building.

### Round 7: finish

As the former round 5. Generate the reference and the support table from the running client
(`eqoa.capabilities`) instead of writing them by hand. Freeze `eqoa/1` only after the round 4 mods
have shipped on it.

### Tools for mod authors (can run beside any round; no API change)

`eqoa mod-test <folder>` (run a mod against a saved snapshot: errors and a screenshot, no login);
`eqoa mod-api` (print the reference from the capability list); a starter template; reload on save;
docs/modding/AGENT-GUIDE.md (one page for agents building mods from an owner brief).
```

---

## Owner decisions

1. **Is "upstream plus mods" the working goal from now on, ahead of FrontiersForge parity?**
   Recommend yes. It unblocks the reordered plan above. Forge compatibility stays; it stops setting the order.

2. **May the HUD extras (Stats Box, Target of Target, Mana Bars, HUD numbers) become bundled mods that are on by default?**
   Recommend yes. Players see the same features; windows become movable; the Target of Target and raid list clash goes away. They may look slightly different drawn by a mod. It unblocks round 3.

3. **The round minimap and the fork's map look: ask Winfro whether vanilla would take them as options (Minimap: Square/Round), or keep them strictly as mods?**
   Recommend asking, as a question and not a PR. If he'd take a Round option, that is the cheapest route by far. If not, they become an alternative mod in round 4. It decides how much of round 4 is needed.

4. **Send the fix PRs now?**
   Recommend yes, starting with the animation crash fix, which is already prepared. It's low risk and shows how Winfro likes to receive changes before the larger seams go out.

5. **Keep the fork's different first-run defaults (NPC display 64, distance 650)?**
   Recommend keeping the behaviour but applying it from the fork's own settings file at first run, so upstream's defaults and tests stay untouched. It unblocks a clean `settings.rs`.

6. **The live-server client check (the answer that stops "Your client is outdated"): tell Winfro about it?**
   Recommend asking him directly. Upstream's rules make what the client tells a server his call, and vanilla players presumably hit the same message. If he takes it, a fork-only network block disappears. If he'd rather not, it stays fork-only. Either way it shouldn't go into a PR unasked.

7. **Take upstream's version where the two are now the same (group waypoint, quest toggles, suggestion form, upstream's new touch controls)?**
   Recommend yes. The in-map suggestion form is the only visible loss, and it can return as a mod. It unblocks the tidy round.

8. **Test builds: should a round that touches every frame be played from a branch build before it reaches Nightly?**
   Recommend yes for those rounds only. It costs one extra download and prevents another build like the under-one-frame-a-second Nightly reaching players.

9. **Should the fork's own mods live in their own repository?**
   Recommend yes once round 3 produces the first four. Mod work then needs no client build, cheap agents can work in parallel, and other authors have real mods to copy. Until then `bundled-mods/` is fine.

10. **Full automation (botting) API: keep it, but last?**
    Recommend yes. It is the costliest part in upstream's files and serves neither syncing nor the fork-features goal. Moving it to round 6 changes nothing that was promised, only when.

---

## What this review did and didn't check

- Read: the fork and sync records, the map comparison, the mod plan, spec sections 1, 2, 5, 7, 9, 10, 12 and 13, the round 2 report, upstream's rules, the backlog headings, the experience log's EQOA entries, and the fork's line-by-line differences in every upstream file.
- Ran: read-only git commands, a dry-run merge of upstream today (nothing written), and the footprint check against `91f9e72` (every budget and hook passes; it reports FAIL only for the stray `du.exe.stackdump`).
- Didn't do: build, run tests, play, or contact upstream. The conflict counts come from the written sync records, not from re-running old merges. Whether Winfro accepts any PR is unknown. Cost estimates are judgement, not measurement.
