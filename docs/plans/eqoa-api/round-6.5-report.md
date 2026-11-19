# Round 6.5 Report: Capabilities, Standalone Window Takeover, `open_loot`/`talk`, and Bag Viewer Auto-Loot

## Fix round 5, 2026-11-18 (base `838e48d0`, develop merged as `d12944fb`): Auto-loot stopped for good

Owner live test (r65-retest3 log): "auto-loot doesn't seem to work; I may have triggered something by trying to loot".
The log: Auto-loot opened corpse 0x187ca (coin only) and released it; corpse 0x187c3 got "Yours alone for 5:12 more." and
"Out of range." in chat and nothing else, a second open of it (the player's own button) got "Out of range." again, both
timed out silently; the player opened a corpse in the game's window twice; corpse 0x187de opened hidden and Take all
worked; then about eight minutes with no auto-loot activity at all. Commits are LoopRook's on `feat/eqoa-api-r6.5`;
nothing pushed or merged. Upstream footprint unchanged: **4 of 6** seam lines (footprint PASS).

Step 0: the fix round 4 agent had left a test-only diff; committed as `47a1970b`. `develop` merged (`d12944fb`): conflicts
in `fork_ext/play.rs` (`sync_hidden_sessions` beside the session recorder's `fork_record::drain`; both kept) and
`FORK-UPDATE-NOTES.md` (both kept). After the merge, round 7's `round7b_support_table_matches_the_running_client` needed the
generated table to list round 6.5's API: regenerated from the built client (`docs/modding/SUPPORT-TABLE.md`,
`eqoa-1-surface.json`; adds `open_loot`, `talk`, `windows.*`, `Entity.npc_flags`).

### 1. Why Auto-loot stopped (diagnosis)

- Reproduction of the owner's exact sequence (`round65_fix5_auto_loot_keeps_going_after_the_owners_sequence`: coin-only
  corpse, "Out of range." and silence with a second unanswered open, the player's own open and close in the game's window
  during a fight, a hidden Take all, then two more kills) **passes on the old Lua** (run against a copy of the pre-fix
  package through `EQOA_R5B_TEST_SOURCE`). No state is left stuck by the no-answer, the player's click or the Take all.
- The fight check was the blocker: Auto-loot waits while "a live NPC has you targeted", and that counted every NPC in the
  zone list: the player's **mercenary** (a group member; the owner's log shows "Your mercenary Quinor is back at your
  side"), a **merchant or banker** spoken to (they keep the player as target), and a creature at the far end of the zone
  that once chased the player. Any of these keeps Auto-loot waiting for good. `round65_fix5_a_friendly_or_distant_npc_targeting_you_is_not_a_fight`
  fails on the old Lua at its first case ("a mercenary targeting you is not a fight", `bag_viewer_tests.rs:4000`).
- Honest limit: the owner's log has no line that names which NPC it was (that is what item 2 adds). The mercenary is
  the likeliest: it was in the group the whole session.
- Fix (`bags.lua` `M.not_a_fight`, used by the zone scan): an NPC with the player targeted is a fight only if it is not
  the player's pet, not in the group (`eqoa.group.get`, now declared), not a merchant/banker/trainer (`npc_flags`
  1/2/8/128), and within 30 units (`M.FIGHT_RANGE`). The game's own combat flag and auto-attack still hold Auto-loot back
  as before (`round65_fix_bag_viewer_auto_loot_waits_while_in_combat` unchanged and passing).

### 2. Always-on log lines

- `eqoa.log.write` now reaches the game's log: every line goes to eqoa.log as `addon <id>: [level] text`; `info` and
  `debug` stop there, `warn` and `error` also become Mods & Addons diagnostics as before (`manager.rs` `mod_log_line`;
  spec line for `eqoa.log.write` updated). Before, every line became a diagnostic, which is why the mod could not log
  routine decisions (the bundled-mod tests require zero diagnostics). This is a small API contract change for the lead to
  confirm.
- The Bag Viewer writes one line per corpse and reason (`bags.lua` `M.alog`): `mode Everything for <name>` /
  `mode Off for <name>: kills are left alone` (when the character's choice is read or changed), `corpse 0x..: <name>, your
  kill: queued for Auto-loot`, `waiting: in combat (<why>)` (the game's flag, auto-attack, or `<name> has you targeted`),
  `waiting: too far (N units; Auto-loot opens within 15)`, `<name>: skipped, not your kill (you did not hit it)` (corpses
  within 40 units), `skipped, already looted`, `opening it (N units away, Take all, try 1 of 3)`, `the server did not open
  it (<reason>): will retry ...`, `gave up after 3 tries`, `done: Take all ok (N items)` / `done: took N of M items (junk
  left)` / `done: no items on it (coin only)` / `done: only junk on it, left there`.
- "mode Off" is logged when the mode is read or set, not per corpse: with Auto-loot off the mod does not listen to creature
  updates at all (round 6.5 fix round 4's frame-rate rule).

### 3. Unanswered corpses are retried

- A corpse Auto-loot asked for that did not open (no answer in 10 s, "Out of range.", a refusal, an immediate refusal) is
  asked again when the player is 2 units closer than at the last try, or 5 s later; three tries in all, then it gives up
  and says so (`M.auto_open_failed`, `M.AUTO_TRIES/AUTO_CLOSER/AUTO_RETRY_SECS`).
- The live server answers a corpse asked for from too far with the chat line "Out of range." and no 0x0085. The broker now
  ends the waiting `open_loot` (and a batch's re-ask of its corpse) as refused with the server's words at once
  (`play.rs` `BrokerReply::OpenLootRefused`, `action_broker.rs` `resolve_server_open_loot_refused`), only while an
  `open_loot` is in flight; the result line shows "Out of range.".
- One corpse open at a time: a second open while the first is unanswered is refused with "still waiting for the server to
  open a corpse" (the owner's second click on 0x187c3 replaced the first request, whose answer then reached nobody).

### 4. Windows that turn a corpse list away (modes 61/62/63 after the 11-18 upstream sync)

- Upstream's `npcw` answers a 0x0018 that arrives outside mode 0 with its own 0x0016 and opens nothing. Before, the
  broker took that list as "opened", and the mod went on to a Take all on a closed corpse ("Take all: cannot open corpse
  right now"). Now, with an `open_loot` in flight and the mode not 0 when the list arrived, the open ends as refused at
  once ("another game window was open; the corpse was closed"), the pending takeover claim is cleared and Auto-loot asks
  again later. Tested here with the start menu (mode 13), which takes the same upstream path as the mercenary windows and
  dye picker.
- **After the sync merge**: re-run `round65_fix5_item4_a_list_the_game_turns_away_ends_the_open` with modes 61 and 63, and
  check the bank action while a mercenary window or the dye picker is up (a held hidden bank session cannot be paused and
  resumed cleanly on this branch: those windows do not exist here). Not done in this round.

### Tests (all in `bag_viewer_tests.rs`)

| Test | Proves | Without the fix |
|---|---|---|
| `round65_fix5_auto_loot_keeps_going_after_the_owners_sequence` | the owner's sequence leaves nothing stuck | passes on the old Lua too (diagnosis, see 1) |
| `round65_fix5_a_friendly_or_distant_npc_targeting_you_is_not_a_fight` | mercenary, merchant, far creature do not block | old Lua: `a mercenary targeting you is not a fight` (`:4000`) |
| `round65_fix5_item2_auto_loot_decisions_are_in_the_game_log` | the lines reach eqoa.log, zero diagnostics | old Lua: `the game log has 'mode Everything for': []` (`:4208`); routing disabled in `manager.rs`: the Addon Manager diagnostic `eqoa/1 [info] auto-loot: mode Off ...` fails `clean()` (`:128`) |
| `round65_fix5_item3_unanswered_corpse_is_retried_when_closer_at_most_three_times` | retry when closer, not standing still, 3 tries | old Lua: fails (no retry, no log; `:198`) |
| `round65_fix5_item3_out_of_range_ends_the_open_at_once_and_is_retried` | "Out of range." ends the open at once, retried closer | Rust arm disabled: still waiting, `assert_eq!(ok, Some(false))` fails (`:4099`) |
| `round65_fix5_item4_a_list_the_game_turns_away_ends_the_open` | refused at once, no loop, asked again after | Rust branch disabled: `Take all: cannot open corpse right now` (`:4143`); old Lua: no retry (`:4152`) |

Negative controls were executed (Lua: the pre-fix package via `EQOA_R5B_TEST_SOURCE`; Rust: each fix disabled with
`false &&` in a scratch edit, run, then restored with `git checkout`, tree confirmed clean). All six pass on the fixed tree.
`tools/test_local_scene.py` (6 tests) passes, including the new `--auto-loot` check.

### Build-tool hazard found (reported to the lead)

All worktrees share one `CARGO_TARGET_DIR` and with it the eqoa test binary's fingerprint. When another worktree builds
after this worktree's last edit, cargo calls the binary fresh and runs the other worktree's code: two of this round's runs
reported "0 passed, 2486 filtered out" (this branch has 2539 tests). The gates in this round were run through a wrapper
that touches `crates/eqoa/src/main.rs` right after taking the build lock (`Paseo/targets/scratch/gate_touch.py`, not in
the repository).

### Gates

Final tree (`de224d81`), `python tools/run_gates.py --release` (through the touch wrapper): **ALL GATES PASSED**.
addons 562 passed, 0 failed, 25 ignored; full 2459 passed, 0 failed, 30 ignored (the usual skips: no `run/client.bin`, no
2002 disc, no live S2/savestates, symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`, mod-api without an
adapter); wasm, footprint (4 of 6 seam lines), paths, player-release and release PASS. `update_bundled_manifest.py
--check` up to date. The oracle tests skip on this machine (no decomp, no eqoa-port oracle).

The first full run failed one older test, `round6_item4_lazy_icons_load_without_native_windows_and_zero_per_frame_requests`
("steady frames must not leak Lua tables", 83,904 bytes). Not a leak: the pre-fix package passed, and the pre-fix package
plus 1,500 never-executed string constants failed too (107 KB). Variants removing `log.write`, `group.get`, the mode
lines, the fight check or all of `alog` all showed the same 83,904 bytes. The single ten-frame window measured Lua's
string table doubling (a collection does not shrink it) and caches settling, which follow the package's code size.
The check now takes three ten-frame windows with a collection after every frame and allows at most one to grow over
1 KB (`de224d81`). Control: a package keeping 1 KB a frame fails (`[-52119, 11710, 11966]`); the old, padded and new
packages pass. Two intermediate forms were tried and rejected because the leak control passed them (one window after a
warm-up; the smaller of two windows).

### Commits (this round)

- `de224d81` test(bag_viewer): the steady-frame Lua memory check measures three windows with a collection after every frame
- `0c8d989e` docs(notes): Auto-loot keeps working, retries unanswered corpses, logs its decisions
- `e91be3c1` chore(bundled): manifest after the Bag Viewer README's Auto-loot paragraph
- `d4acd2bc` docs(mod-api): regenerate the support table and surface with round 6.5's API (open_loot, talk, windows.*, npc_flags) after the develop merge
- `cc03ce5b` feat(tools): local_scene --auto-loot checks Auto-loot's log lines
- `4b2c169a` fix(takeover): a corpse open ends at once on the server's "Out of range." or when the game turns the list away (another window up, modes 61-63 after the upstream sync)
- `ccaed037` feat(addons): eqoa.log.write lines go to eqoa.log; info and debug stay out of the Mods & Addons diagnostics
- `cb543061` fix(bag_viewer): Auto-loot no longer stops for good: a mercenary, a merchant or a far creature with you targeted is not a fight; unanswered corpses retried (closer or 5 s, 3 tries); every decision logged
- `47a1970b` test: per-character Auto-loot test drives sync_character directly; H1 single-take keeps the corpse held (left uncommitted by fix round 4)
- plus the develop merge `d12944fb` and this report.

### Not tested here (needs the live server)

- Auto-loot through real fights with the mercenary out, and the real server's "Out of range." retry.
- The local scene's GM `#kill` is not the player's hit, so it cannot show a real auto-loot; `--auto-loot` checks the mode
  line and the "not your kill" line end to end.
- `python tools/local_scene.py --scene bag-viewer --r65 --auto-loot` against the local server with this build:
**PASS**, 21 of 21 checks (run `20261119-035207-73e21148`), including `auto-loot: mode Everything but junk for Lsgm` and
`auto-loot: corpse 0x186e0: fire bat: skipped, not your kill (you did not hit it)` in the real client's log.

## Fix round 4, 2026-11-18 (base `909fe54e`): frame rate, hidden re-opens, Auto-loot choice, greyed-out buttons

Owner retest of `909fe54e`: "feels sluggish" (24 fps typical, 13 low; `hud` 10-40 ms a frame, worst with the Bags window
open), and Auto-loot and batches still showed the game's corpse window. Owner-approved UI changes (2026-11-18) are items 3-6.
Commits are LoopRook's on `feat/eqoa-api-r6.5`; nothing pushed, merged or tagged. Upstream footprint unchanged: **4 of 6**
permitted seam lines; every change is in fork-owned files (`window_takeover.rs`, `play.rs` fork hooks, the bundled mod,
tests).

### 1. Frame rate: cause and fix

**Cause (from the owner's `eqoa.log`, `EQOA_PERF` lines).** With the Bags window open the log shows
`eqoa_bag_viewer 7.4-10.0 ms (lua 1.9-2.5, api 5.4-7.4)` and `addon-api: eqoa_bag_viewer entities.list 4.0x 5.1-6.7`:
the window asked for `eqoa.entities.list()` **four times a frame** (`visible_tabs` and the button row each asked
`nearby_corpse`; the context-button test asked `nearby_npc("merchant")` twice, the bank row once more), and every call clones and
compares each creature record of the zone (`read.clone`, `record.list` in the `core:` line, 3-10 ms with the window open). The
fight check of Auto-loot (`in_combat`) scanned the whole zone again on every creature update while a corpse waited. With
the window closed the mod cost 0.2-0.5 ms; the `wereoxxs_ui` Forge package alone costs 6-8 ms of the quiet `hud` 10 ms
(not part of this round).

**Fix (`bags.lua`, `main.lua`).** One scan of the zone (`scan_zone`) answers the nearest corpse, merchant and banker, the
corpses auto-loot may take and the "live hostile has me targeted" check. It is kept 0.5 s and dropped at once when a
creature is added or removed (events the open window listens to), or turns into a corpse or changes its npc flags or target
(the auto-loot handlers). Auto-loot's creature handlers are registered only while Auto-loot is on (the default is off), so the
closed window no longer receives every creature update of the zone.

**Test.** `round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame`: 100-slot pack, 700 creatures (35 updates a
frame), a corpse and a merchant in reach. The window must ask for the list at most 3 times a second and cost less per frame
than one list call (any build), and in a release build under 3 ms open and 0.2 ms closed.
`round65_fix4_item1_waiting_auto_loot_shares_one_scan_of_the_zone`: 200 questions about the zone make one scan.

**Before / after (the Bag Viewer's own time per frame, `addons:` line of the `EQOA_PERF` report).**

| | before (old Lua) | after |
|---|---|---|
| Owner's real zone, Bags window open (release, his log) | 7.4-10.0 ms (api 5.4-7.4: `entities.list` 4 calls, 5.1-6.7 ms) | about 2-3 ms, the window's own drawing (estimate, not measured live) |
| Test scene, 700 creatures, 87 updates a frame, window open, release | **63.8 ms** (53.6 list calls/s, 4 a frame) | **3.0 ms** (no list call in the count) |
| Test scene, 700 creatures, 35 updates a frame, window open, release, machine busy | **60.0 ms** (70.8 list calls/s) | **5.3 ms** (one scan counted, 0.7 ms of it) |
| Same, window closed, release | 0.43-1.28 ms (the creature handlers) | 0.02-0.83 ms (0.02 with 35 updates a frame: no creature handlers while Auto-loot is off) |
| Debug build (what the gates run), window open | 223 ms | 16-18 ms, under one list call (28-32 ms) |

The test scene's list is about 8 times dearer than the owner's real one (one call 7-15 ms against his 1.7 ms), which is why the
"before" is larger than his log; the number of calls a frame (4 before, none or one in two seconds after) is the same cause. What
is left when the window is open is the drawing of its 48 visible slots (about 2 ms of Lua in release); the numbers are from the
release-profile test binary on a machine other agents were using, so they vary 2x between runs.

### 2. Auto-loot and batch re-opens showed the game's corpse window

**Cause (owner log).** After a hidden `open_loot` the live server, after every take, sends the item, the chat line, the
close (`0x0016`) **and lists the corpse again by itself** (`0x0018`); the log shows `observed None` for each such list, so no
client request (`0x0015`) preceded it. The first open's claim had been used up by the first list, so each later list
had no claim and was left to the game ("no request of the holder is waiting for this window"), and the game's window showed
between the takes. The broker's re-ask (`0x0015`) is a second path (it arms its own claim and already worked); the real
server does not wait for it.

**Fix.** `window_takeover::decide_open`: a list for the corpse the holder's hidden session is on, with no request of the
player's own in between, is that session going on (the idle timer renews). `note_loot_request` marks a loot request no claim
of the holder stood behind as the player's own (`player_asked`), so the player clicking that same corpse still gets the
game's window. `play.rs` `sync_hidden_sessions`: the session is kept across the server's close while the holder's batch
waits for the corpse to be listed again.

**Tests** (`bag_viewer_tests.rs`): `round65_fix4_item2_take_all_but_junk_relists_stay_hidden`,
`round65_fix4_item2_auto_loot_relists_stay_hidden` (server lists again at once, three takes, no `left to the game` line, game
mode stays 0, three HIDDEN decisions), the same two with the broker asking (`..._reopens_asked_by_the_broker_stay_hidden`),
and `round65_fix4_item2_a_players_own_click_on_the_corpse_still_shows_the_game_window`.

### 3-6. Owner-approved UI changes (bundled Bag Viewer)

3. **Auto-loot** is a choice **Off / Everything / Everything but junk** (three buttons under the loot log, a one-line
   explanation under them), saved with the junk marks in `junk.cfg` (`auto=off|all|non_junk`). It replaces the on/off option
   (the Options page no longer lists one; `eqoa.options.*` capabilities dropped from `mod.json`). Everything is one Take all
   (junk included); Everything but junk is the old batch. The junk marks file is the addon's own data file; the mod does not
   key it by character today, so "per character" is exactly as per character as the junk marks are. To make room for the
   explanation line the loot log shows 5 lines instead of 6 (the window must stay inside the 448-pixel HUD frame; the
   existing `round6_item5_window_fits_hud_and_scrolls_100_slot_pack` test caught it).
4. **Labels**: `Look at corpse` (preview only, neutral border) and `Take all` / `Take all but junk` (green when they can act).
5. **"Also sell grey items"** removed with its setting, rule and code; only marked items are junk. An old file's `grey=` line
   is ignored and not written back.
6. **Out-of-range buttons grey out.** `Sell junk`, `Open bank`, `Deposit`, `Withdraw` show "too far from the merchant" /
   "too far from the banker" (and clicking says the same, nothing is sent) when the nearest merchant/banker is beyond the
   reach the server last accepted. The reach is learned per session from real answers: the largest distance that got an
   answer (`ok`) and the smallest that got none (`fail`, only for the broker's ten-second silence); also from a window the
   player opens by hand. Between a known-good and a known-bad distance the button is allowed; until anything is known the
   default is 10 units (`M.DEFAULT_REACH`). An open merchant/bank window is never blocked.
   The Bv test harness now gives the Lua the player's position (it follows `bags_player_at`; before, the snapshot had none
   and every distance read 0).

**Tests**: `round65_fix4_item3_*` (Off leaves the corpse; Everything takes the junk; Everything but junk leaves it; three
buttons, explanations, saved across reload, old option gone), `item4_look_and_take_labels`, `item5_no_grey_items_option`,
`item6_out_of_range_buttons_grey_out_and_the_reach_is_learned` (30 units: greyed; 5 units: answer learned; 9 units: silence
learned; between: allowed; open shop: not blocked), `item6_bank_rows_follow_the_reach_and_native_opens_teach_it`. Changed to
follow the new rules: the junk-marks persistence test (grey checkbox gone), the merchant sell-junk batch test (the Rat Pelt
is marked, not "grey"), the unanswered-bank test (the first silence teaches that 8 units is too far; the second visit stands
at 4), the "Open nearest corpse" labels, and `bv_auto` instead of the option.

### Executed negative controls (this round)

| Item | How the fix was removed | Result |
|---|---|---|
| 1 | old package (`EQOA_R5B_TEST_SOURCE` = the committed Lua) | `item1` window test fails: "asks for the zone's creature list 53.5 times a second" (release), 19.7 (debug) |
| 3 | old package | `item3_*` (4 tests) fail: no `auto_mode` / no choice buttons |
| 4 | old package | `item4` fails: buttons were `Open nearest corpse`, `Also sell grey items`, ... |
| 5 | old package | `item5` fails: "no grey-items checkbox: [Also sell grey items]" |
| 6 | old package | both `item6` tests fail: "greyed out and says why: Sell junk (1 item)", "14 units: past the default reach" |
| 2 | `window_takeover.rs` and `play.rs` put back to `909fe54e`, rebuilt | the two `relists` tests fail ("the game's corpse window must not show (list 1)", mode 1); the two `asked` tests and the player-click test pass without the fix (they guard the second path and the player's own window) |

### Gates

Pending: the full `python tools/run_gates.py --release` result is added below when the run ends.

### Not tested here

The server's real behaviour (that it lists the corpse by itself after each take, the reach at which merchants and bankers
answer, the default of 10 units) is read from the owner's log and not reproduced; the live server decides. The oracle tests
skip on this machine (no decomp). Owner checklist: `Paseo/targets/r65-checklist-3.txt`.

---

## Owner-test recovery, 2026-11-18 (base `20b3eebb`)

The earlier gate results below describe the build the owner tested, not acceptance
of this recovery. Live owner QA is pending. The interrupted builder's eight-file
uncommitted diff was preserved under ignored `run/`, reviewed, corrected, and
completed on `feat/eqoa-api-r6.5` as LoopRook. No push, merge, tag, disc data, or
server changes are part of this recovery.

### Changes and causes

1. **Hidden corpse reads:** the items snapshot already understands hidden sessions,
   but scope-own takeover depended on the global loot-source queue being untainted.
   Old unanswered requests could keep that queue ambiguous, so a mod's later open
   fell through to the native window. `window_takeover::PendingOpen` now observes
   the requests sent after arming, including their entity id. A conflicting request
   in that interval still declines takeover; ordinary player opens remain native.
   The separate source-name tracker stays conservative: ambiguous source names are
   not fabricated. The rendered Corpse tab and `eqoa.loot.get()` read the same items.
2. **Display versus taking:** the button is **Open nearest corpse**. Opening only
   displays the corpse; it leaves no take queued. Take all, Take all but junk, item
   menus, and the separate Auto-loot checkbox retain their existing taking paths.
   Existing round 5/6 take, refusal, Stop, and per-take server-close coverage stays.
   Button widths and padding fit the full labels at the default width; the
   rendered fixture lists three items and takes nothing.
3. **Merchant:** the previous API, spec, Lua discovery, and fixture metadata called
   bit `0x08` a merchant. The actual native interaction sends `0x004C` for bit
   `0x01`; bit `0x08` sends smith/repair `0x0061`. Discovery and takeover arming now
   use `0x01`, and the erroneous immediate "trainer completed" branch is removed.
   The existing `Play::interact_with_target` is called unchanged. The new test
   records exact outbound `0x004C + npc_id` bytes, skips a closer smith, and waits
   for matching `0x00B7` before success. Silence becomes "merchant did not answer"
   after the broker's existing 300 ticks (ten seconds), rather than leaving
   "Opening merchant..." indefinitely.
4. **Diagnostics:** always-on fork-side `takeover:` lines record dispatched mod
   open_loot/talk entity ids and opcodes, accepted/declined takeover decisions
   (including already-native windows), and hidden-session close reasons.
5. **Close once:** outgoing close packets and local `play: 0x0016` handler echoes
   are different. The old cleanup called `npcw_opcode(0x0016)` even after the server
   had already cleared the corpse, and again when releasing a holder with no
   session. This explains repeated log entries without proving repeated outgoing
   packets. Close obligations now carry whether a real session existed and why it
   ended; cleanup skips both the packet and native handler when already closed.
   An accepted distant corpse also stays readable: the old immediate 25-unit
   corpse-distance rejection is removed. Walking more than 25 units from the
   position where a hidden session opened still ends it, as does idle expiry.
6. **Bank:** unanswered opens and moves now end in the result line, mentioning
   the banker and possible distance. Silence does not establish a range refusal,
   so the wording is a question; actual refusal text continues unchanged through
   the existing result path. The server still decides range and permissions.

### Verification record

| Area | Implementation location |
|---|---|
| Request observation and hidden open decision | `crates/eqoa/src/fork_ext/addons/window_takeover.rs:362`, `:384` |
| Always-on decisions and closes | `crates/eqoa/src/fork_ext/play.rs:419`, `:545` |
| Hidden session lifetime | `crates/eqoa/src/fork_ext/play.rs:596` |
| Native merchant target and dispatch | `crates/eqoa/src/fork_ext/play.rs:1552` |
| Corpse display-only action | `bundled-mods/eqoa_bag_viewer/modules/bags.lua:512` |
| Merchant/banker timeout results | `bundled-mods/eqoa_bag_viewer/modules/bags.lua:1005`, `:1014` |

Always-on lines go through `window_takeover::log` (stderr, plus a test-only
buffer), so Rust tests assert the exact lines (request entity and opcode, accept,
decline with reason, close with reason). An earlier Python log checker inside the
crate was removed: it needed a built test executable and belonged in no test run.

Recovery note: the first builders were cut off with their implementation lost from
the tree by an interrupted negative control. It was restored from the saved stage
snapshots, reviewed hunk by hunk, and re-verified. The recovery commits are
`badf6702`, `1fb74cb6`, `5a720448`, `11bf2ccc`, `f5bc4bac`, `403ce60d` (one per fix
item, in that order: hidden corpse items, button label/display-only, merchant,
logs, one close per session + far corpse, bank/timeouts) plus the log-helper
commit that follows them.

First full gate run (before the last fix): `round6_m1_loot_all_ignores_unrelated_inv_add_before_close` failed, because the new "no answer" wording replaced the broker's own reason ("corpse closed") for a take the server never confirmed. Fixed in the Bag Viewer (`M.timed_out`: only the broker's 10 s timeout gets the new wording); that old test is the control. `round4_device_recovery_flushes_json_...` failed once under load and passes alone.

### Gates (recovery, final tree)

`python tools/run_gates.py --release`, full, no filter: **ALL GATES PASSED**. addons 507 passed / 0 failed; full 2402 passed / 0 failed / 30 ignored (disc-dependent tests that skip here: no 2002 disc, no live S2, no `run/client.bin`, no symlink privilege, no reviewed UI goldens); wasm, footprint (4 of 6 upstream lines, unchanged), paths, player-release and release build PASS. Oracle tests skip on this machine (no decomp). Live server behaviour (merchant answer, bank range) is not covered by any test.

### Executed negative controls (recovery)

Each fix was reverted in the working tree and the `round65` tests run; all 28 pass
with the fixes. With a fix reverted, these fail (first failing assertion):

| Fix reverted | Failing test |
|---|---|
| request-tracked claim (stale loot bookkeeping) | `round65_fix2_open_corpse_shows_items_hidden_even_with_stale_loot_requests`: "the game's corpse window must not open" (mode 1) |
| Open button takes (`loot_all`) | `round65_fix2_open_button_only_displays_never_takes`: result "Take all: queued..." not "Corpse ready" |
| merchant flag 0x01 to 0x08 | `round65_fix2_merchant_uses_native_flags_and_exact_request`: smith 0x0061 chosen, no 0x004C |
| 0x0016 sent even if the server closed | `round65_fix2_one_close_per_hidden_session`: 2 closes, expected 1 |
| close with no session | same test, part (c) |
| 25-unit corpse-distance check back | `round65_fix2_far_corpse_session_is_not_closed_at_once` |
| unanswered result text (merchant, bank open, bank move) | `round65_fix2_unanswered_merchant_talk_fails_visibly`, `round65_fix2_unanswered_bank_requests_fail_visibly` |
| each log line removed (open_loot, talk, accept, decline, close, release-without-session) | the open, merchant, M2 and one-close tests |

No upstream line was added or changed: **4 of the 6 permitted seam lines remain
in use** (see `round-6.5-seams.md`). All implementation stays in fork-owned files.
Bundled file checksums are regenerated with the mod changes.

The native player-initiated windows, live merchant response/range behavior, bank
range/refusal wording, and ownership permissions still need the owner's server
test. Protocol-shaped offline replays cannot establish live server acceptance.
The decomp/oracle is unavailable on this machine; no oracle result is claimed.

---

Branch: `feat/eqoa-api-r6.5` from `develop` (`9046c28b`), reviewed footprint base `a76d8d27a1c3f51fca545d7eabaca9e5dc336091`. Commits are LoopRook's; nothing was pushed, merged, or tagged.

## Commits

| Commit | Summary |
|---|---|
| `f6bbf3c6` | `feat(addons): drop one-gesture rule and enforce action capabilities` |
| `9e46ea78` | `feat(addons): add window takeover, open_loot, talk, and npc_flags` |
| `29d07f6a` | `feat(bag_viewer): standalone loot/merchant/bank takeover and auto-loot` |
| `e737a3de` | `docs(api): round 6.5 spec, getting started, seams, update notes, and report` |
| `b1ebec96` | `fix(addons): skip takeover TLS probes when idle and reset hidden sessions in tests (L4, L5)` |
| `0d625967` | `fix(addons): match npc_id when resolving talk server open (L2)` |
| `2465752a` | `fix(addons): clear corpse visibility and master-loot state on hidden loot release (L3)` |
| `a9b0c566` | `fix(api,addons): scope window takeover to mod-started sessions and release on stale (M1-M3)` |
| `06ea4c10` | `docs(api): fix talk/npc_flags spec and document takeover scoping and limits (L1, L6)` |
| `b3c11196` | `feat(api): add own/all takeover scope to eqoa.windows.takeover (M1 amendment)` |
| `01a48721` | `fix(addons): require explicit capability list for actions and takeover (not_declared)` |
| `edf30939` | `fix(addons): takeover claims need a known matching target, owed closes, target restore, no hiding of open windows (M1 Rust, M2, M3, L4)` |
| `70b58da4` | `fix(bag_viewer): auto-loot only after you hit it and out of combat; Loot tab renews its session (M1, L5, L6)` |
| `6d73a1a8` | `fix(bag_viewer): declare exactly the actions the mod uses; spec and notes for the review fixes (L7)` |

## What Was Delivered

### 1. Drop One-Gesture Rule + Capability Enforcement & UI Labeling
- Removed per-call player-gesture gating (`needs_player_action`) across `action_broker.rs` and `native_state.rs`; gameplay actions (`eqoa.actions.*`) and `eqoa.windows.takeover` require explicit manifest capability declarations (`eqoa.actions.*`, `eqoa.actions.<name>`, `eqoa.actions.batch`, `eqoa.windows.takeover`).
- Mods without a `capabilities` list in `mod.json` (`DiscoveredAddon::declared_capabilities() == None`) or bare `Host::new()` sessions receive a visible `not_declared` refusal + diagnostic (while reads keep their current behavior); mods with a `capabilities` list lacking the capability receive `permission_denied`.
- Added `Host::requested_capabilities(&self)` (`host.rs`) and `Session.capabilities` / `capability_grants` / `require_capability` (`native.rs`).
- Derived `autonomous_action_label(caps)` and Mods & Addons manager detail lines (`runtime.rs`) from the same `declared_capabilities()` source (`Actions: can act on its own (all actions)` or `Actions: can act on its own (<kinds>)`).

### 2. Window Takeover (`scope = "own" | "all"`), `open_loot`, `talk`, and `Entity.npc_flags`
- **`crates/eqoa/src/fork_ext/addons/window_takeover.rs` & `native_r6a.rs`**:
  - Added `eqoa.windows.takeover(kind, opts?)` (`opts.scope` = `"own"` (default) or `"all"`), `eqoa.windows.release(kind?)`, and `eqoa.windows.taken(kind)` for `"loot" | "merchant" | "bank"`.
  - Requires declared capability `eqoa.windows.takeover` (`permission_denied` otherwise).
  - With `scope = "own"` (default), only sessions opened by the holding mod's own `open_loot`/`talk` (or batch corpse reopen) are hidden; player-started opens still show their normal native windows (`mode == 1 / 10 / 11`).
  - With `scope = "all"`, every session of that kind (including player-started opens) is hidden (`mode == 0`) for mods drawing a full replacement window (documented that such mods must draw their own UI and give the player a close control via `release`).
  - Hidden `scope = "own"` sessions store the request's `target_id` (M2: preserving banker ID across retargeting between `talk` and `0x124D`) and auto-end when the mod's action/batch completes, on walk-away (>25 units), or after a 15 s idle timeout (M3). Hidden `scope = "all"` sessions stay open across single transactions until released, walked away (>25 units), or zoned.
  - Zero-addon / idle fast path (`HAS_ACTIVITY` `Cell<bool>`, L4) performs zero TLS borrows on `window_taken`, `bank_taken_tick`, `sync_hidden_sessions`, and `*_session_open`; `reset_all_for_test` resets all hidden sessions and activity flags (L5).
  - Second holder returns `nil, Error{code="busy"}` and records a diagnostic naming the current holding addon.
  - Auto-releases on disable, reload, VM error (`release_vm`), and zoning (`release_all`), closing hidden native sessions cleanly (`close_hidden_session`, sending `0x0016` and clearing `loot.visible`, `loot.ml`, and master-loot state on loot release, L3).
- **`crates/eqoa/src/fork_ext/play.rs` & `items_capture.rs`**:
  - Added `window_taken`, `try_accept_open`, `sync_hidden_sessions`, `NoopRaster`, and `bank_taken_tick` so hidden loot, merchant, and bank sessions keep native `mode == 0`, draw nothing, and steal no controls while routing `npcw` and `bankplay` actions (`0x1254`/`0x1255`).
  - Added native `0x0085` string-table error word lookup so server refusals expose readable reasons.
  - Exposed creature `npc_flags` (`npc_flags: uint?`, `nil` without a creature record) via `api_capture.rs` and `api_updates.rs`.
- **`crates/eqoa/src/fork_ext/addons/action_broker.rs`**:
  - Added `eqoa.actions.open_loot(target?)` (corpses only; nearest corpse via `ee` math when omitted; sends `0x0015`; correlates `0x0018` / `0x0085`).
  - Added `eqoa.actions.talk(target?, opts?)` (`kind = "merchant" | "bank" | "any"`; refuses corpses and NPCs missing the requested `npc_flags`; nearest matching NPC via `ee` math when omitted; sends the native target-interaction opcode `0x004C`/`0x124D`/`0x0061`/`0x0004`; correlates `0x00B7` matching `npc_id` [L2], `0x124D`, or `0x0085`).
- **Known Limits (`EQOA-API-SPEC.md`)**:
  - (L3) On packet arrival, the upstream open sound cue plays before the fork takeover hook suppresses the window.
  - (L6) Generic window actions (`select_row`, `activate_row`, `dialog_answer`, `menu_command`) gate on `play.mode` and are refused on hidden/replaced windows (`mode == 0`); mods use direct actions (`loot`, `loot_all`, `buy`, `sell`, `bank_move`, `bank_money`) instead.

### 3. Upstream Seam Accounting (`round-6.5-seams.md`)
- Total upstream lines added: **4** inserted one-line hook calls across 2 files (`0` upstream lines deleted or rewritten; pre-approved ceiling 6), documented in `docs/plans/eqoa-api/round-6.5-seams.md` and `docs/fork-footprint.json`:
  - `crates/eqoa/src/npcw.rs` (2 lines): `0x0018` and `0x00B7` takeover mode suppression.
  - `crates/eqoa/src/bankplay.rs` (2 lines): `0x124D` takeover mode suppression and `run_bank` `bank_taken_tick` entry hook.

### 4. Bag Viewer (`bundled-mods/eqoa_bag_viewer`) & Auto-Loot
- Updated `mod.json` (declared `eqoa.windows.takeover`, `eqoa.entities.*`, `eqoa.player.get`, `eqoa.target.get`, `eqoa.options.*`), `modules/bags.lua`, `main.lua`, and `bundled-mods/SOURCE.txt`.
- **Bag Viewer (`bundled-mods/eqoa_bag_viewer`)** uses `scope = "own"` only: instead of holding global takeovers while idle/open, it acquires takeover around its own actions/batches (**Loot**, **Take all**, **Take all but junk** including Sandstorm per-take reopen, **Sell junk**, **Open bank**, **Deposit**, **Withdraw**) and releases on batch/action completion, refusal, zoning, or panel close, leaving manual player clicks on corpses/merchants/bankers in their normal native windows.
- Added an **Auto-loot** option (default OFF) that automatically opens and loots non-junk items from killed targets becoming nearby corpses under `scope = "own"` without opening the native loot window.
- **Known Auto-Loot Limitation**: EQOA's native protocol does not transmit kill credit or corpse ownership flags to the client, so Auto-loot tracks the player's targeted living NPC and enables auto-loot only when that tracked entity dies/turns into a nearby corpse while Auto-loot is ON. Corpses left by other players or already-dead NPCs are skipped by Auto-loot (manual **Loot** still works on any nearby corpse).

### 5. Documentation
- Updated `docs/modding/EQOA-API-SPEC.md` (`Entity.npc_flags`, `eqoa.windows.takeover/release/taken`, `eqoa.actions.open_loot`, `eqoa.actions.talk`, capability enforcement replacing the one-gesture rule).
- Updated `docs/modding/GETTING-STARTED.md` (capability requirements in place of one-gesture wording).
- Updated `FORK-UPDATE-NOTES.md` (`## Unreleased` player-visible notes for autonomous action labels, standalone Bag Viewer takeover, and Auto-loot).

## Review Fixes (Sonnet review: APPROVE WITH CHANGES, no HIGH)

Each fix has a test that failed before it. Rust fixes were written test-first and the failure below is that first red run; the Lua fixes were checked by running the new tests against the previous Lua (`EQOA_R5B_TEST_SOURCE` pointing at a copy of the old package). Items that share code are in one commit (M1 Rust with M2, M3, L4; M1 Lua with L5, L6). L4 needed no new upstream line: the total stays 4 of 6.

| Item | What changed | Test | Failure without the fix |
|---|---|---|---|
| M1 retarget | `open_loot`/`talk` record the player's previous target (`window_takeover::note_retarget`). When the mod is done (no pending open, no hidden session, nothing queued, in flight or running: `action_broker::vm_is_idle`) and no native window is up, the previous target returns through the game's own target request, unless the player chose another target or it is gone or a corpse. The spec documents the target change and the restore. | `round65_fix_m1_previous_target_restored_when_the_mods_open_is_done` | With the restore call disabled: `M1: the player's previous target comes back` left 9001, right 9500 (`items_tests.rs:3829`). |
| M1 combat | Auto-loot (Lua) does not run while in combat and goes on when the fight ends. The signal is the game's own combat flag (`player.fighting`, the one that asks for battle music), auto-attack on, or a live NPC whose `target_id` is the player (the best signal the client has: the wire has no "being attacked" flag beyond these). `player.changed` re-runs the held-back loot. | `round65_fix_bag_viewer_auto_loot_waits_while_in_combat` | Old Lua: `M1: no auto-loot while a live hostile has you targeted` left 1, right 0 (`bag_viewer_tests.rs:2583`). |
| M2 | Under scope own a pending open claims an open only when its target is known to match: loot and merchant by the observed id, bank by the player's current target. An unmatched open stays native and the pending claim survives for its own reply. A pending claim whose request is refused, timed out or gone is dropped at the next iteration (`action_broker::open_requests_waiting`). `scope = "all"` is unchanged. | `round65_fix_m2_pending_open_never_claims_a_player_started_open`, `round65_fix_m2_refused_open_clears_pending_claim` | `overlapping loot requests: the reply must not be claimed by the pending open` left 0, right 1 (`items_tests.rs:3655`); `a refused talk must not leave a pending claim on the player's own open` left 0, right 10 (`items_tests.rs:3712`). |
| M3 | `sync_sessions_with` records an owed close per kind (`close_owed`); it is taken and sent only when `mode == 0`, otherwise it stays (and keeps the takeover state active) until a native window no longer blocks it. Corpse, `LOOT_BUSY` and master-loot state are cleared when it is sent. | `round65_fix_m3_hidden_session_end_while_mode_nonzero_is_deferred_not_dropped` | `the owed close must keep the takeover state alive` (`items_tests.rs:3737`); before the fix the close was consumed and never sent. |
| L4 | `window_taken` declines when the window's mode was already current before the packet (`MODE_BEFORE_EVENT`, set in `begin_game_event`, cleared in `end_game_event`). | `round65_fix_l4_window_already_open_is_not_hidden_on_arrival` (loot, merchant, bank) | `L4: an already-open native corpse window must not be hidden by the reply` left 0, right 1 (`items_tests.rs:3770`). |
| L5 | `eqoa.windows.takeover(kind)` called again while holding renews a live hidden session's idle timer; the Loot tab calls it every frame it is shown. Walk-away, zoning, release and panel close still end the session. | `round65_fix_bag_viewer_loot_tab_keeps_hidden_session_alive` | Old Lua: `L5: the Loot tab must renew the session while it is shown` (`bag_viewer_tests.rs:2638`). |
| L6 | Auto-loot's "my kill" signal: you hit the creature (an outgoing damage `combat.hit` that is not your pet's, or your auto-attack on it while targeted). Not a player's or pet's corpse, not one nobody on your side touched, not another player's kill. Still uncertain (the wire carries no kill credit or loot rights): a creature you hit that another player then killed counts, and the server's refusal (shown in the result line) decides; manual **Loot** works on any corpse. | `round65_fix_bag_viewer_auto_loot_only_corpses_of_creatures_you_hit` | Old Lua: `a corpse of a creature you did not hit is not auto-looted` left 1, right 0 (`bag_viewer_tests.rs:2521`). |
| L7 | `mod.json` declares `eqoa.actions.open_loot, talk, loot, loot_all, sell, bank_move, use_item, equip, quick_slot, batch` instead of `eqoa.actions.*`, so the Mods & Addons label reads "can act on its own (open_loot, talk, loot, loot_all, sell, bank_move, use_item, equip, quick_slot, batch)". `buy` and `bank_money` are not called by the mod, so they are not declared (`cancel` and `stop_batch` ask for no capability). The bundled manifest was regenerated. | `round65_fix_bag_viewer_manifest_declares_exactly_the_actions_it_uses`; the label assertion in `round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal` | With the old `mod.json`: `no blanket eqoa.actions.*` (`bag_viewer_tests.rs:2671`). |

The three tests the review asked for: pending open beside a player-started open (`round65_fix_m2_pending_open_never_claims_a_player_started_open`), hidden-session end while `mode != 0` (`round65_fix_m3_...`), window already open on arrival (`round65_fix_l4_...`).

Changes to older tests, forced by the new rules and not weakening them: the bank step of `round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` no longer retargets before `0x124D` (an unmatched bank open is now native; that case is covered by the M2 test), `round65_loot_release_clears_corpse_visibility_and_master_loot` records its loot request so the reply is known to match, `activate_hidden_for_test` passes the player's target for banks, `round65_bag_viewer_auto_loot_option_takes_all_but_junk_on_killed_target` makes the kill a hit of yours, and the label assertion names the declared actions.

Needs the live server: the restore after a mod's open (the server's order of the retarget and the loot request), the owed close under a player's native window, the combat flag in a real fight, and Auto-loot after a real kill.

## Executed Negative Controls

Each fix was temporarily broken in source, tested against the focused regression test to observe failure, and cleanly restored:

1. **Capability gate (`eqoa.windows.takeover`)**:
   - **Fault**: Temporarily bypassed `require_capability(lua, "eqoa.windows.takeover", ...)` in `crates/eqoa/src/fork_ext/addons/native_r6a.rs`.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65 -- --test-threads=1` failed `play::fork_hooks::api_capture::items_tests::round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` at `crates/eqoa/src/fork_ext/items_tests.rs:2844:10` (`assertion failed` verifying undeclared capability denial).
2. **Native window takeover hook (`window_taken`)**:
   - **Fault**: Temporarily forced `window_taken` in `crates/eqoa/src/fork_ext/play.rs` to return `false`.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2879:5` (`assertion 'left == right' failed: taken-over loot must keep mode == 0; left: 1, right: 0`).
3. **Bag Viewer server refusal handling across dispatch phase (`bags.lua`)**:
   - **Fault**: Prior to filtering `d.phase ~= "dispatch"` in `bundled-mods/eqoa_bag_viewer/modules/bags.lua` `M.on_action_result`, dispatching `open_loot` cleared `M.pending_open` before `0x0085` arrived.
   - **Observed failure**: `cargo test --locked -p eqoa --bin eqoa round65_bag_viewer -- --test-threads=1` failed `round65_bag_viewer_standalone_loot_sell_bank_without_native_windows_and_refusal` at `crates/eqoa/src/fork_ext/addons/bag_viewer_tests.rs:2333:5` (`left: Some(true), right: Some(false)` instead of displaying server refusal text).
4. **L4 zero-addon zero-probe fast path (`window_takeover.rs`)**:
   - **Fault**: Forced `window_takeover::has_activity()` to return `true`.
   - **Observed failure**: `cargo test -p eqoa round65_zero_addon_zero_takeover_probes` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3172:5` (`left: 64, right: 0`).
5. **L5 test reset clears hidden sessions (`window_takeover.rs`)**:
   - **Fault**: Skipped clearing `hidden` in `window_takeover::reset_all_for_test()`.
   - **Observed failure**: `cargo test -p eqoa round65_reset_all_for_test_clears_hidden_sessions` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3202:5` (`assertion failed: !window_takeover::has_activity()`).
6. **L2 talk `0x00B7` `npc_id` correlation (`action_broker.rs`)**:
   - **Fault**: Bypassed `opened_npc_id` equality check in `resolve_server_talk_open`.
   - **Observed failure**: `cargo test -p eqoa round65_open_loot_talk_npc_flags_and_strict_target_guards` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3136:168` (`wrong npc_id must not confirm talk`).
7. **L3 hidden loot release cleanup (`play.rs`)**:
   - **Fault**: Bypassed `play.npcw_opcode(0x0016, &[])` cleanup in `close_hidden_session`.
   - **Observed failure**: `cargo test -p eqoa round65_loot_release_clears_corpse_visibility_and_master_loot` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3237:5` (`corpse must be cleared`).
8. **M1 `scope = "own"` does not hide player-started opens (`window_takeover.rs`)**:
   - **Fault**: Accepted opens without checking `pending_opens` under `scope = "own"`.
   - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2895:5` (`player-started loot must open native window even under takeover; left: 0, right: 1`).
9. **M1 amendment `scope = "all"` hides player-started opens (`window_takeover.rs`)**:
   - **Fault**: Disabled the `holder_scope == TakeoverScope::All` branch in `try_accept_open`.
   - **Observed failure**: `cargo test -p eqoa round65_window_takeover_scopes_own_and_all` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3503:5` (`scope=all must hide player-opened loot window; left: 1, right: 0`).
10. **M2 bank target preservation across retarget (`window_takeover.rs`)**:
    - **Fault**: Stored `7777` instead of `pending.target_id` when accepting `Bank` hidden session.
    - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:2990:5` (`M2: hidden banker_id must come from talk request (9003), not retargeted play.target (7777); left: Some(7777), right: Some(9003)`).
11. **M3 walk-away release (`play.rs`)**:
    - **Fault**: Bypassed position distance checks in `sync_hidden_sessions`.
    - **Observed failure**: `cargo test -p eqoa round65_window_takeover_keeps_mode_zero_draws_nothing_and_auto_releases` failed at `crates/eqoa/src/fork_ext/items_tests.rs:3029:5` (`M3: walk-away must end hidden loot session`).
12. **Explicit `capabilities` list requirement (`native.rs`)**:
    - **Fault**: Restored `let Some(caps) = s.capabilities.as_ref() else { return Ok(()); };` in `require_capability`.
    - **Observed failure**: `cargo test -p eqoa round65_undeclared_capabilities_refuses_actions_and_takeover_with_not_declared` failed at `crates/eqoa/src/fork_ext/addons/action_broker.rs:4775:14` (`assist without capabilities list must refuse with not_declared`).

## Gate & Verification Results (`EQOA_ISO=run/eqoa.iso`)

After the review fixes, `python tools/run_gates.py --release` (no `--filter`, no `--skip-step`): **ALL GATES PASSED**. The `enhanced_report_transport_does_not_follow_redirects` flake did not occur in this run.

| Step | Result | Details |
|---|---|---|
| `addons` | PASS (500.09s) | 503 passed, 0 failed, 25 ignored (9 distinct skips: Windows symlink privilege not held x4, `EQOA_FORGE_SPIKE` missing x2, `EQOA_BUNDLED_UI_GOLDENS` missing x3) |
| `full` | PASS (2026.22s) | 2395 passed, 0 failed, 30 ignored (30 distinct skips: `run/client.bin`, `EQOA_2002_ISO`, external Sandstorm/go-eqoa-server patch folders, `1.6-savestates`, Windows symlink privilege, `EQOA_FORGE_SPIKE`, `EQOA_BUNDLED_UI_GOLDENS`) |
| `wasm` | PASS (21.05s) | `cargo check --locked -p eqoa --target wasm32-unknown-unknown` |
| `footprint` | PASS (2.30s) | `python tools/check_fork_footprint.py --base a76d8d27a1c3f51fca545d7eabaca9e5dc336091` (no budget change: 4 of 6 upstream lines) |
| `paths` | PASS (5.30s) | `python tools/check_no_local_paths.py` |
| `player-release` | PASS (42.03s) | `python tools/test_player_release.py` |
| `release` | PASS (178.34s) | Release binary built cleanly |

Also: `python tools/update_bundled_manifest.py --check` passes after the manifest regeneration. The oracle tests skip on this machine (the decomp and eqoa-port oracle are not here); the 2002-disc, live-S2 and pinned-Forge tests skip as listed above.
