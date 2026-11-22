# Round 8 plan: offer the neutral `eqoa/1` hooks upstream, prepared alongside A1

Written 2026-11-22 by the Fable planner. Planning only: no code changed, nothing built, nothing sent anywhere.
Baseline: `develop` a83afe4d (footprint base 35e2a43b). `sync/2026-11-21` (base 2d4c8449) is not merged yet and
`origin/master` is 7f2597c0, 4 commits past it. `file:line` is on a83afe4d; paths are under `crates/eqoa/src/`
unless a crate is named. Sources: the plan, FORK.md, the footprint file, the UI audit and window design,
upstream's guide as refreshed on `sync/2026-11-21`, and the fork's diff against upstream read line by line.

## Report

**Summary.**
1. The fork has 907 added and 138 deleted lines in 43 of upstream's files. About 200 of them (22 files) are
   neutral seams any mod host could use. The rest is fork features, five bug fixes and about 30 leftover lines.
2. No hook line can go upstream as written: 127 of them name `fork_`, `forge_` or Enhanced, and the adapters
   reach `Play`'s private fields through `#[path]` child modules that upstream does not have.
3. Recommended: one offer, delivered as a stack of small PRs, after a one-page note to the maintainer
   (upstream's guide says "ask first"). The smallest useful PR is the game-event observer plus the overlay layer.
4. **Licence correction:** the brief says upstream is MIT. It has been AGPL-3.0-or-later since 2026-11-07
   (ab32f96), the same licence as this repository. Nothing is relicensed; the owner still has to agree to
   contribute the code (section 2).
5. A1 should be built and owner-tested before the PR that carries window seams and before the final freeze.
   The fixes and the observer PR do not depend on A1 and can go first.
6. The final freeze is a date on three files and a release, set after upstream's answer and A1's test (section 3).
7. Seven work packages can run overnight without the owner. Sending the note, pushing PR branches, opening
   PRs, A1's spec and the freeze release each need the owner.

**Files changed and commits.** This file only, one commit on `docs/round8-upstream-pr-plan`.

**Verified (rerun from the worktree; `G` is `git -c safe.directory=*`).**
- Fork lines in upstream's files: `G diff --numstat 35e2a43b HEAD -- <the "shared" paths of docs/fork-footprint.json>`
  gives 907/138 in 43 files. Every hunk in `crates/` was read from `G diff -U0 35e2a43b HEAD -- crates/eqoa/src
  crates/net/src crates/render/src crates/vi/src` (fork-owned paths excluded): 1,232 diff lines.
- Upstream's licence: `G show origin/master:LICENSE` (AGPL v3) and `G show origin/master:Cargo.toml`
  (`license = "AGPL-3.0-or-later"`); its NOTICE keeps MIT only for commits up to 1e23e80.
- Dry-run merge `G merge-tree --write-tree --name-only develop origin/master`: 5 conflicts (.gitignore, hud.rs,
  play.rs, pregame/station/api.rs, viewer.rs); the same command from `sync/2026-11-21`: none.
- Four of row J's fixes are still absent upstream: `G grep` at origin/master for the pad, texpack, clip and
  Station lines found upstream's unfixed text. The fifth (the power-bar reset) was not checked.
- Frozen record: 1,654 supported paths; surface file: 1,654 supported, 9 partial, 89 unsupported, 6 `not_yet`.

**Not checked.** No build or test was run. The Forge project's own licence. Whether the mod runtime builds
on Linux or macOS (the fork's CI is Windows only; upstream ships all three). Where `replaces` is enforced in
code (the 0.7.0 notes say it is built). The A1 seam at "about play.rs:10450" is the audit's number, not re-read.
`viewer.rs:4273` (`p.keyboard = true`) was not traced to its reason. Line counts per row are counted by hand
from the diff and are approximate.

## 1. Inventory: what the fork has in upstream's files

Class: **N** neutral, offer it. **N+** neutral in form but only a mod host uses it. **X** a bug fix that
stands alone. **F** the fork's own, stays. **L** leftover, delete in the fork.

| # | Seam | Where | Lines | Class | What upstream would need |
|---|---|---|---:|---|---|
| A | Game-event observer | play.rs :47 (module), :224, :3600, :3602, :3662 and :4325 (before and after each server event), :4060, :6579, :6622, :7843 | 11 | N | Neutral names; a no-op default in a file upstream owns; a recording test (a scripted fight seen in order at 30 and 165 Hz); the `guide.hit` reorder at :7832-7841 put back as upstream wrote it |
| B | Exact health numbers kept beside the game's | play.rs :394, :4082, :4207, :4263, :10140, :10232; hud.rs :24, :423 | 17 | N+ | Not offered by itself: fold into A (the observer sees the same packets) or keep it in the fork |
| C | "Is this HUD part shown?" at the draw and at the click | hud.rs :417-432, :477, :534, :575, :612, :652, :687, :706; play.rs :5941-5992, :10356-10384, :10506-10535; viewer.rs :696, :6164, :7026 | 31 | N | One shared question answered in vanilla by Hide UI and its options; same-picture test; per part, its quads and click area go and nothing else. A4's four hide seams replace most of these calls: design both together |
| D | A layer over the finished frame, offered input first | viewer.rs :24, :1887, :3248, :3714, :3870, :3936, :4051, :5444, :5453, :5479, :6876, :7036 | 28 | N | A slot upstream owns in `Gfx` instead of the fork's concrete `forge_runtime` type; kept through a lost graphics device; no draw and no allocation when empty |
| E | Read-only window state | bank.rs :157; bankplay.rs :26; classmastery.rs :598; dialog.rs :27, :113, :348; itemmenu.rs :362; loot.rs :110; merchant.rs :267; npcw.rs :22, :32; questlog.rs :107; spellbook.rs :186; startmenu.rs :164; trainer.rs :182 | 52 | N | A caller in vanilla (an unused `pub fn` in a program crate is a compiler warning): one test each, or they travel with the mod host |
| F | Requests in, window claimed | npcw.rs :164, :202, :480; bankplay.rs :103, :188, :222; play.rs :3601 (camera), :6333 (which corpse), :600, :7774, :8059 (Summary returns to play) | 16 | N+ | The maintainer's yes to mods that act; travels with the mod host |
| G | Option rows, saved settings, Controls rows | options.rs 22/12 (`pages()` six times, `items` twice, change, activate, value text, session begin and cancel); settings.rs :51, :716, :1844, :2146, :3177; bindpage.rs 7/3 | 43 | N | A refactor of upstream's own tables: each row declared once, ids 1000 and up kept for other builds, saved file byte-identical. The largest ask and the largest sync saving |
| H | Map overlay | map/mini.rs :643, :650, :656; map/mod.rs :88, :2086; crates/render ui.rs :588 | 8 | N | Neutral names; picture unchanged |
| I | Host entry and dependencies | main.rs :187, :464; Cargo.toml (mlua, three dear-imgui crates); Cargo.lock +153 | 160 | N+ | Only with the mod host |
| J | Fixes | crates/vi csprite.rs 85/11 and hsprite.rs 69/3 (animation bounds); pregame/station/api.rs :161-242 and its test (a reply read before the close); input.rs 32/3 (no phantom controller); texpack.rs :2077 (Windows test path); crates/net session.rs :2628 (power-bar reset, to confirm) | 245 | X | One small PR each with a test that fails first. `vi` changes need the oracle case in the same change, and the oracle is not on this machine |
| K | Fork features | Local reports and redaction (logfile.rs, crashfile.rs, main.rs :222, :732, :739, Station `secret` calls and log lines, hud.rs :316, :487, play.rs :6000, :6533, about 12 viewer.rs lines, settings.rs :248, startmenu.rs :696); release channel (update.rs, updatewnd.rs); icon (build.rs, icon.rs); live-server client check (net session.rs about 75 lines, s2.rs); recorder (net lib.rs :15, session.rs :2488); chat history key (play.rs :5132, :9951); first-run defaults (settings.rs :2166); compass waypoint marks (hud.rs :735-748); NOTICE, .gitignore | 245 | F | Nothing. The client check is a question the owner puts to the maintainer directly (plan decision 6) |
| L | Leftovers | ui.rs `with_font` and `font_override` (10/6, no caller); main.rs :163 and the 3-line statbox.rs; reworded upstream comments (play.rs :5326, :5435, :9948; viewer.rs :4994; hud.rs :308, :683; net session.rs :3269); play.rs `world_id()` edits (:9862-10035) | 30 | L | Nothing: restore upstream's text in the fork |

## 2. What the PR contains

**Shape: one offer, a stack of branches.** The plan folded the seam PRs into "one combined offer" (owner,
2026-11-10). Keep one offer and one conversation, but deliver it as a stack: the mod host is about 100,000
lines, upstream's `master` reaches every player at their next start, and its guide wants every change tested
with a negative control. Each PR below is cut from `origin/master`, leaves the game unchanged for a vanilla
player, and can be accepted or declined alone.

| Step | Content | Rows | Size in upstream's files | Depends on |
|---|---|---|---|---|
| Note | One page from the owner: what is offered, what a vanilla player sees (nothing), the measured cost with no mod (the zero-addon gate), the questions in section 5 | - | - | Owner sends |
| PR 0 | The fixes, one PR each | J | about 245 lines with tests | Nothing |
| **PR 1** | **Observer and overlay: the smallest acceptable set.** A host can watch the game and draw over it. No Lua, no new dependency | A, D (B folded in) | about 45 call lines, one new neutral file, tests | The note answered |
| PR 2 | HUD parts, window state, map overlay, plus A1's two diversion lines | C, E, H | about 95 lines | PR 1; A1 owner-tested |
| PR 3 | Option, settings and Controls rows declared once | G | a refactor of options.rs and settings.rs | PR 1's reception |
| PR 4 | The mod host: Lua runtime, `eqoa/1`, Mods & Addons window, docs, generated support table, frozen record and its guard test. Native only, no mod bundled, nothing runs until a player enables one | F, I and the fork's runtime files | about 25 lines and Cargo.lock; the rest is new files | The maintainer wants it |
| PR 5 | FrontiersForge compatibility layer | - | new files | Only if asked; licence check first |

**Stays in the fork:** every bundled mod, row K, the Enhanced options page, the first-run defaults, autonomous
helpers shipped as mods. Whether mods may act on their own in vanilla is the maintainer's call; in the host it
is one switch, not an API change.

**Upstream's rules each branch must meet.** Neutral names (no `fork_`, `forge_`, Lua or mod names in PR 1 to 3).
Never `cargo fmt`; never reorder or reflow upstream's lines; stage files by name. Tests with an executed
negative control; frame-counted behaviour tested at 30 and 165 Hz. The browser build (`wasm32`) and all three
desktop systems must build. No game data. A PR that changes what a player sees adds its section to
`UPDATE-NOTES.md` in the same push (PR 4 does; PR 0 to 3 do not). Upstream's files carry no licence header.

**Licence (owner decides).** Both projects are AGPL-3.0-or-later; code published through Enhanced 0.4.5 is MIT,
which AGPL accepts. What would be offered under upstream's licence, as "DabDavis and contributors":
- PR 0 to 3: the fork-written lines of rows A to H and J, their tests and the new neutral file.
- PR 4: `fork_ext/addons/**` (83,888 lines, 23,086 of them tests), the adapters `fork_ext/play.rs`,
  `api_capture.rs`, `api_updates.rs`, `items_capture.rs`, `r6a_capture.rs`, `player_stats.rs`, `hud.rs`,
  `bindpage.rs`, `map_api.rs`, `mods.rs`, `modtools.rs`, the mod parts of `fork_ext/options.rs` and
  `settings.rs`, `docs/modding/**`, `tools/eqoa1_freeze.py`.
- Third-party code comes as dependencies under its own licence: mlua and Lua 5.1 (MIT), dear-imgui-rs, -wgpu
  and -winit (MIT or Apache-2.0).
- Not offered: bundled mods (their authors' own), fonts and map data, and the Forge layer until its origin is
  checked.

The owner must agree to three things: the contribution cannot be taken back; no extra condition can ride along
(the retired 0.4.6 clause about Sandstorm cannot be attached to AGPL code); the fork's per-file licence line is
dropped in upstream's copy, so credit is the commit author and "contributors" unless the maintainer adds a line.

## 3. The `eqoa/1` freeze

Today (0.7.0) `eqoa/1` is a "stable candidate": the guard test `eqoa1_frozen_surface_only_grows` is on and
`docs/modding/eqoa-1-frozen.json` lists 1,654 supported names. The record guards names and status only. Types,
units, error codes and event order are promised in `VERSIONING.md` and held by the round 7 field and docs tests.

**"Final freeze at the upstream PR" means, concretely:**
1. The note and PR 1 have gone out and the maintainer has answered, or the waiting time has run out (question 6).
2. Any correction agreed with him, and any that A1 forces, is made. These are the last breaking changes
   `eqoa/1` ever gets; each is listed in the release notes.
3. `SUPPORT-TABLE.md` and `eqoa-1-surface.json` are regenerated (the committed surface file still says client
   0.6.0) and `python tools/eqoa1_freeze.py update` is run. From then on `--allow-downgrade` is used only for
   a name found to be fake, with the owner's approval and a player note.
4. `VERSIONING.md`'s status line and the spec's header change from "stable candidate" to "frozen on <date>, in
   Enhanced <version>". Proposed: `eqoa.revision` stays 1 at the freeze and rises by one in each later
   release that adds names.
5. A Stable release carries it, and `FORK-UPDATE-NOTES.md` tells third-party authors they can build on it.
6. If upstream takes PR 4, the record and the guard test go with it, and the two projects agree who may add
   names (proposed: additions land in one place first and the other syncs them).

**Settle before the PR goes out:**
- `sync/2026-11-21` merged, so the base is current and the dry-run merge is clean.
- The one "under review" item in spec section 8 (line 901) becomes supported or `not_yet`.
- The six `not_yet` names stay out: `move`, `look`, `buttons` (after the PR), `drop_quest`, `mastery_buy`,
  `mastery_refund` (with the confirm contract). Adding them later is an addition, not a break.
- The round 7 contract rules as built are re-read: an unknown window kind answers `unsupported`, `all` needs
  `replaces`, `own` is refused on views.
- Two policies that could change what a supported call returns in upstream's build: mods acting on their own,
  and servers that ask for the game as it was. The owner's answers (2026-11-17, 2026-11-19) hold for Enhanced;
  the note asks the maintainer, and a refusal of type `unavailable` is already an allowed answer.

## 4. How A1 fits

**What A1 needs from upstream:** two inserted lines in play.rs: the first line of `draw_hud` (:10185) and one
in the window pass before the Start menu and Items draw (about :10450). The confirm contract, which brings
back `drop_quest` and adds `destroy`, is expected to need none: destroy is the item menu's own request (0x003C)
carried out by `item_outs` (play.rs :7455), and the game's yes/no box is already readable (`MBox::sel`,
dialog.rs :348). Check in the round. A1 changes the per-frame path, so the owner plays a branch build first.

**Recommendation: build A1 beside the PR preparation; send the fixes and PR 1 first; send PR 2 and freeze
after A1's owner test.** Reasons:
1. The freeze is final. The confirm contract defines what an irreversible action means, and round 7H already
   had to correct the yes/no rules just before the guard went on. A lesson from A1 costs nothing before the
   freeze and costs `eqoa/2` after it.
2. One complete ask is better than three. With A1 built, PR 2 carries its two lines, proven by a shipped mod,
   instead of the fork returning weeks later for two more lines, then eight for the HUD round.
3. A mod replacing the game's bags is the best evidence that "upstream plus mods" works.
4. Nothing waits. The note, PR 0, PR 1 and all preparation need nothing from A1, so the conversation with
   upstream starts now and its answer can still shape A1.

This differs from the note of 2026-11-22 in the audit ("after the round 8 upstream PR"); the owner decides
(question 2). If the owner prefers the PR first, PR 2 goes without A1's lines and the freeze still waits for A1.

## 5. Risks and owner questions

Risks: (a) Upstream moved 8 to 45 commits a day over the last two weeks; a PR branch goes stale in days, so
branches are small and re-cut by a tool. (b) The maintainer may decline; the cost is the preparation, and the clean-up in WP2 lowers
the footprint either way. (c) Neutral names mean the fork renames its side once after a PR is accepted. (d)
The runtime has only ever been built on Windows. (e) The animation fixes change code ported line by line from
the original game, which upstream may refuse without its oracle. (f) Freezing before upstream answers could
freeze a name it wants changed.

Questions for the owner:
1. **Licence.** May the code listed in section 2 be given to the vanilla client under its licence (AGPL, the
   same as ours), for good and with no extra conditions? For which steps: PR 0 to 3 only, or the mod host too?
2. **Order.** Build A1 (bags, spellbook, journal, the confirm) before the window PR and the final freeze, as
   recommended, or send everything first?
3. **Ask first.** Will you send Winfro a one-page note before any PR? An agent drafts it; you send it.
4. **How much to offer.** The seams only, or also the whole mod host (Lua, the Mods & Addons window, the API)?
   Recommended: offer both and let him take the seams first.
5. **Fixes.** You dropped the fix PRs on 2026-11-10. Offer them now as a warm-up (crash guard for broken
   animations, a refused login showing its reason, no phantom controller, a Windows test path)?
6. **Waiting time.** If Winfro has not answered, freeze anyway after how long? Recommended: 14 days.
7. **If vanilla wants limits.** If he wants mods in vanilla unable to act on their own, or to obey servers
   that ask for the game as it was, is a switch for that acceptable in his build? Enhanced stays as it is.
8. **Who presses the button.** PR branches must be pushed to your GitHub fork and each PR opened under your
   name. Will you do each one, or give the lead a go-ahead per PR?

## 6. Work packages

Every brief carries the plan's "Rules for every worker brief". PR branches are cut from `origin/master` in
their own worktree, named `upstream-pr/<name>`, and stay local. Reviewer: a different model family.

| WP | Work | Files | Tests and verification | Overnight? |
|---|---|---|---|---|
| 1 | Inventory check (scout, read-only). Re-read every row of section 1 on develop after the sync and at `origin/master`; list each fork name in a call line; confirm the fixes are still missing upstream; re-read the contract rules and the A1 seam places | `docs/plans/eqoa-api/round-8-inventory.md` | Lead spot-checks ten lines | Yes |
| 2 | Clean-up on develop: delete row L; restore upstream's wording and order where the fork only reworded. Keep a line when `git blame` shows a test needs it, and say why | ui.rs, main.rs, statbox.rs, play.rs, viewer.rs, hud.rs, crates/net session.rs, the footprint file | `python tools/run_gates.py --release`; footprint PASS with budgets lowered to measured; dry-run conflict count before and after | Yes |
| 3 | PR tool: cut and re-cut a PR branch from `origin/master`, refuse fork names, formatting churn, game data and unstaged-by-name files, run upstream's tests. It never pushes | `tools/upstream_pr.py`, `tools/test_upstream_pr.py`, `docs/DEV-TOOLS.md` | Its unit tests, with a planted fork name as the negative control | Yes |
| 4 | PR 0 branches, one per fix, without the fork's report or log lines; the pad test renamed in upstream's style | Row J files | Each test run failing without its fix, then passing; that crate's upstream tests; oracle skips counted | Yes |
| 5 | PR 1 branch: neutral observer and overlay slot, no-op by default | play.rs, viewer.rs, one new file, tests | Recorder test at 30 and 165 Hz; each call removed in turn fails a test; nothing allocated when empty; `cargo check --target wasm32-unknown-unknown`; upstream's full tests | Yes, then review |
| 6 | Trial: develop plus PR 1, the fork's matching hook lines deleted and `fork_ext` moved onto the neutral names. Not merged; kept as the patch to apply when PR 1 is accepted | A trial branch from develop | Full gates `--release`; the three bundled UIs and the fork's mods load; fork lines in upstream's files before and after | Yes, after WP5 |
| 7 | The note and the PR descriptions in the owner's voice, from the direction review section 3, with measured numbers | `docs/plans/eqoa-api/round-8-offer.md` | `tools/check_no_local_paths.py`; lead reads it | Yes |
| 8 | Mod-host map (scout, read-only): which `fork_ext` files are host, API, Forge layer or fork feature; what crosses; what Linux and macOS need | `docs/plans/eqoa-api/round-8-host-map.md` | Lead spot-check | Yes |
| 9 | A1: the contract, bags, spellbook, journal, the confirm with `destroy` and `drop_quest` (window design W1) | `fork_ext/**`, two lines in play.rs, the Bag Viewer mod | The design's test list; owner branch build | Owner approves the five-line spec first |
| 10 | PR 2 branch | Rows C, E, H and A1's two lines | Same-picture test; per-part hide tests; one test per accessor | After WP9's owner test |
| 11 | PR 3 branch (design reviewed by the lead before building) | options.rs, settings.rs, bindpage.rs | Upstream's page walk tests unchanged; saved file byte-identical; a planted duplicate id fails | After PR 1's reception |
| 12 | PR 4 branch | From WP8's map | Zero-addon gate; all three systems build; docs tests | Only when the maintainer wants it |
| 13 | Freeze and release (section 3) | Docs, frozen record, VERSION, notes | Full gates; `eqoa1_freeze.py update`; owner test | Owner's go-ahead |

**Needs the owner each time (outward-facing):** sending the note; pushing any `upstream-pr/*` branch to
GitHub; opening each PR; the client-check question; the freeze release. After a PR is accepted, the next
`python tools/sync_upstream.py` run applies WP6's patch, deletes the matching fork lines and lowers the budgets.
