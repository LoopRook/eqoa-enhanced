# Test automation study: how much of the owner's testing can agents take over?

2026-11-17, branch `docs/round-7-audit`. Planning only: no game code changed. File and line numbers are
this branch's unless a branch is named.

## Answer in short

Most of what the owner's tests have caught so far could have been caught without him (five of the seven
findings in the table below, fully or partly). Three things stay his: how the game feels, whether a feature is what he wanted, and the go-ahead for a release.

The biggest finding: the server the owner tests on is **eqoaclassic.com** (the round 6 test package's
`config/server.opt` and its `eqoa.log` both say so), and that server's program is public:
`github.com/DabDavis/eqoa-server` (AGPL-3.0, Go, last pushed 2026-11-17). Its loot code states, in a
comment, the exact behaviour that failed the round 6 owner test: after each single take the server closes
the corpse (0x0016) and sends it again (0x0018). A copy of that server on this PC would have shown the
round 6 bug before the owner played. The round 6 report and tests call that server "Sandstorm"; the
sequence may be Sandstorm's too (the server's comment says it follows Sandstorm), but the owner's
package was connected to eqoaclassic.com.

## What slipped past the offline tests so far

| Round | What the owner found | Kind | Catchable without him by |
|---|---|---|---|
| 2 | Under 1 fps in the world with no mods on | Frame rate | A scripted run in the world that reads the fps log (options 2 and 4) |
| 2 | Wereoxxs Enter key, stacked icons, Tab reaching a mod | Live data and input | Partly: a scripted run with real server data; key feel stays his |
| 4 | fps numbers per mod in a busy spot | Frame rate | The fps log collected automatically (option 5); a repeatable busy spot on a local server (option 4) |
| 6 | Server closes the corpse after every take; loot log empty | Server behaviour | A local copy of the server (option 2); a recording made it a test afterwards (option 1) |
| 6 | Icons missing until a game window was opened | Real data | A scripted run with the real disc and real items (option 2) |
| 6 | Window off screen with a 100-slot pack | Real data | Same; the 100-slot pack is a server offer the offline fixtures did not have |
| 6 | "Take all" hard to find | Design | The owner only |

## The options, ranked by value against cost

| Rank | Option | Catches | Cost | Needs from the owner |
|---|---|---|---|---|
| 1 | Test package collects its own evidence, with a checklist (5) | Nothing new, but every test yields logs, fps and a pass/fail list without questions | One small worker job, Python only | Nothing |
| 2 | Session recorder (first half of 1) | Makes every live-only bug diagnosable from the first test | One worker job, fork-owned Rust, no upstream lines | Nothing (recordings stay on his PC) |
| 3 | Local server with a scripted smoke run (2) | Server behaviour, real data, crashes, before the owner plays | One larger worker job to set up, small per round after | Yes or no to installing Go and the server here |
| 4 | fps comparison and screenshots from that scripted run (4) | Frame-rate regressions; broken or missing UI | Small once 3 exists | Nothing |
| 5 | Replay tests from recordings (second half of 1) | Regressions in what the client reads from the server | One worker job, then a little per round | Nothing |
| 6 | In-game checklist overlay (5) | Nothing new; shortens the test and records results | One worker job, as a mod | Approval of the 5-line UI spec |
| 7 | Scripted client on the live server (3) | The same as 3, on real data | Almost none technically | Permission from the server's owner; see below. Not recommended |

### 1. Recording live sessions and replaying them

What exists:

- The client always logs nine loot, merchant and bank opcodes with their sizes
  (`crates/eqoa/src/npcw.rs:122-127`). That line is what solved round 6.
- `EQOA_NET_DEBUG` (`crates/net/src/session.rs:1625`, printed at `:2268-2291` and `:2355-2365`) logs every
  datagram and message, but only headers and the first 12 to 16 bytes: enough to see order, not to replay.
- Every server message already passes a fork hook: `Play` takes the session's events
  (`crates/eqoa/src/play.rs:3335`) and calls `fork_hooks::begin_game_event` for each
  (`play.rs:3367`, `crates/eqoa/src/fork_ext/play.rs:88`). A recorder there needs no new upstream line.
- What the client sends can be kept too: `Session.record` (`session.rs:1456`, filled at `:1666` and `:1676`).
- Replay has precedent. Tests build the game on a fake connection (`Session::connect_with`,
  `session.rs:1534`; the rig in `crates/eqoa/src/autoplay.rs:246-258`), deliver one message
  (`session.rs:3628`), and upstream replays a real PS2 capture into the network layer
  (`crates/net/tests/dump_capture.rs:47-80`).

What to build: a fork-owned recorder, on by default in test packages only, that writes each server
message and each sent message (time, type, bytes) to its own file beside `eqoa.log` (the log itself is
capped at 32 MB, `crates/eqoa/src/logfile.rs:16`). Then a replay helper that feeds a recording to the
existing test rig.

Limits, stated plainly:

- A replay is passive. It proves the client reads a recorded sequence correctly. Once a fix changes what
  the client sends, the recording cannot answer differently. It suits loot logs, item lists and state
  reads; it does not replace a server for back-and-forth flows.
- Recordings hold character names and other players' chat. They stay on the owner's PC, like the disc;
  tests that need them skip elsewhere. Committed tests are small hand-cut sequences, as round 6's were.
  So "every session becomes a fixture" really means: every session becomes a local recording the lead
  can search and replay, and the lessons from it become committed tests.

Value: high for diagnosis. Round 6's first fix round reported two of six items "not reproducible
offline"; a recording would have answered them at once. Medium for regression.

### 2. A local server

In reach, and closer than expected.

- Public source, AGPL-3.0. One program serves login (9735), the game (10070) and patches (7000). It
  uses a pure-Go SQLite, so it builds on Windows with no C compiler. Go is not installed on this PC.
- Content: an "open database" is published with the server (about 6 MB packed, latest 2026-11-14). It
  has the world's creatures, items, quests and loot, and no accounts.
- Accounts can register themselves on first login (`auth.allow_auto_register`); a GM account can spawn
  creatures and use `#` commands, which makes repeatable scenes possible.
- Optional: ground and model files baked from the owner's own disc (about 100 MB, needs about 2 GB of
  memory once). Without them creatures walk through walls; fine for UI and loot tests.
- The client already points anywhere: `eqoa play <host> <user>` (`crates/eqoa/src/main.rs:671`), password
  from `EQOA_PASSWORD` (`main.rs:674`), ports from `EQOA_AUTH_PORT` and `EQOA_GAME_PORT` (`main.rs:857-859`).
- Upstream tests this way itself: its notes describe scripted runs against "the local server"
  (`docs/upstream/CLAUDE.md:326`, `:355-359`).

How close to live: the same program family as eqoaclassic.com as of its last public push. Three
differences remain: the live database (the owner's characters, his 100-slot pack, live-only content),
the live settings, and any commits not yet pushed. It says nothing about Sandstorm, whose server is
closed; players on Sandstorm stay covered only by recordings and reports.

Cost: one larger worker job to install Go, build, fetch the database, write a start script and one
scripted scene (log in, kill a GM-spawned creature, loot it with the bag viewer, take a picture, quit).
After that, a scene per round is a small addition. Nothing from the server goes into this repository:
not its code, not its database.

Unknown until tried: whether the client's scripted mouse and keys reach mod windows as a real click
does. The scripts exist for the game's own windows (`EQOA_MOUSE`, `crates/eqoa/src/mouse.rs:819`;
`EQOA_KEYS`, `crates/eqoa/src/pregame/host.rs:434`; `EQOA_PAD`, `crates/eqoa/src/input.rs:473`;
`EQOA_TARGET`, `crates/eqoa/src/viewer.rs:2476`; `EQOA_USE`, `viewer.rs:1005`; `EQOA_SAY`, `viewer.rs:2458`).

### 3. A scripted client on the live server

Technically it works today with the same commands as option 2. The question is not technical:

- It is an automated client on someone else's server. Whether that is allowed is the server owner's
  call, and the owner of this fork would have to ask. Nothing here assumes it is allowed.
- The risk falls on the owner's account and on this fork's standing with that server.
- Upstream notes one login at a time per account, and that a killed run holds its session about a
  minute (`docs/upstream/CLAUDE.md:357-358`), so it would also block the owner from playing.

Recommendation: do not do this. Option 2 runs the same server program without touching anyone's server.
If a live-only check is ever needed, the owner asks the server's owner for a test account first.

### 4. Screenshots, picture comparisons and frame time

What exists:

- Offline pictures of mod UIs on made-up game state: `bundled_picture_tests.rs:98`,
  `round_3_picture_tests.rs:271`, `bag_viewer_tests.rs:754` (all under
  `crates/eqoa/src/fork_ext/addons/`). The exact-match comparison is skipped: the owner looked at the
  captures on 2026-11-10 and did not approve them as references (plan Results, round 2.5).
- Pictures of the real game: `EQOA_OFFSCREEN=WxH` draws without a window (`viewer.rs:5044`) and
  `EQOA_SHOTS` saves frames at set times (`viewer.rs:1452-1470`; the pregame too,
  `pregame/host.rs:503`); `EQOA_QUIT` ends the run (`viewer.rs:2414`). These need to be in the world,
  so they need a server: option 2.
- Frame time: under `EQOA_PERF` the client prints fps, worst frame and a per-phase breakdown every five
  seconds (`viewer.rs:2048`, line format at `:2112`), and each mod's cost
  (`fork_ext/addons/addon_perf.rs:35`, `:251-263`). Both land in `eqoa.log`. The test package has a
  second launcher for it (`tools/make_owner_test_package.py:280-287`).
- The gates skip the timing tests (`tools/run_gates.py:17-23`); only the "no mods costs nothing" check
  runs in CI.

Gaps and what to do:

- The round 6 package's two kept logs hold no fps lines: the fps launcher was not the one used for
  those runs, so the numbers were lost. Fix in option 5: one launcher that always logs fps, if a quick measurement shows
  the logging itself costs nothing noticeable.
- Exact-match pictures break between graphics cards (the owner's PC already fails two of upstream's
  lighting tests that pass in CI). Better use of pictures: the lead looks at them before the owner does,
  and a loose comparison flags "this window vanished" or "this moved a lot". The lead can read PNGs.
- A repeatable fps number: the scripted scene from option 2, run on the old build and the new one in
  the same spot with the same GM-spawned crowd, on the owner's own graphics card. That replaces "go to a
  busy spot and read the number" for regressions. It cannot stand in for a real raid.

### 5. Shorter owner tests

- **Automatic collection.** The lead and the test package are on the same PC, so nothing needs sending.
  The package's `config/` already holds `eqoa.log` and the run before it (`logfile.rs:1-5`), the state
  snapshot `state.txt` (`fork_ext/report_core.rs:223`), crash files (`report_core.rs:479-499`) and, in
  the addon folder, `addons_error.log` (`fork_ext/addons/manager.rs:459`). Two gaps: only one earlier
  run is kept, so a third launch erases the first; and nobody summarises them. A collector script fixes
  both: the launcher sets logs aside per run, and `tools/collect_owner_test.py` prints one page (build,
  server, fps lines, opcode lines, mod errors, crashes).
- **F3 is already an evidence button.** It saves a zip with a screenshot, a note and the log to the
  Desktop (`fork_ext/report.rs:579-590`). The checklist can say "press F3 when something looks wrong";
  the collector picks the zips up.
- **A checklist per round.** The round's brief lists what changed in player terms; the packager writes
  it as `CHECKLIST.txt` in the package, numbered, each step a single action and what should be seen.
  The owner answers with numbers ("3 fails"). Steps a scripted run already passed are left off.
- **An in-game overlay.** Best built as a small mod on this fork's own mod API: shows the current step,
  one key for pass, one for fail, writes the result to its own folder for the collector. No upstream
  lines. It is player-facing UI, so the owner approves a 5-line description first. Worth doing after
  the paper checklist has proved which steps matter.

## What will always need the owner

- Feel: input delay, whether a window reads well at his screen size, whether motion looks right.
- Fit: whether the feature is what he meant ("Take all" was there, but not where he looked).
- Real crowds: a full raid, a busy town with other players, long sessions.
- Anything that depends on his own characters and their belongings on the live server.
- Decisions: server rules, releases, pushes, what goes upstream.

With options 1 to 4 in place, his test of a round should be the checklist's judgement steps only,
on a build that has already logged in, looted, sold, banked and held its frame rate by script.

## Recommended first step (one worker job)

**Test package, version 2: it collects its own evidence.** Python only, no game code, no cargo build
beyond what the packager already does, safe to run beside round 7.

1. `tools/make_owner_test_package.py`: the launcher sets the previous run's `eqoa.log`,
   `addons_error.log` and `state.txt` aside in `results/<date-time>/` before starting, so no run is
   lost. Keep both launchers unless a measurement shows the fps log is free; then make it the default.
2. `--checklist <file>`: copies a round's checklist into the package as `CHECKLIST.txt`.
3. New `tools/collect_owner_test.py <package>`: one page per run with build commit, server, fps lines
   (lowest, typical, worst frame), the opcode lines, mod errors, crash files and any F3 zips on the
   Desktop newer than the package. It prints paths relative to the package and never copies account
   files.
4. Tests in `tools/test_make_owner_test_package.py` and a new test file for the collector, each with a
   failing control. `python tools/check_no_local_paths.py` must pass.

Then, in order: the session recorder (option 1, first half), the local server with one scripted scene
(option 2, after the owner says yes to installing Go), fps and pictures from that scene (option 4),
replay tests, and the overlay last.

## Not verified in this study

- Nothing was built or run. The local server was not installed; its closeness to live is read from
  its public source and notes, not measured.
- Whether scripted input reaches mod windows is untested.
- The cost of always-on fps logging was not measured.
- The decomp and oracle are not on this machine, as in every round.
