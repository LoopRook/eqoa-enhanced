# Round 7 ("finish") plan: audit fixes, generated support table, author docs, freeze of `eqoa/1`

Written 2026-11-17 by Fable for the Opus lead. Planning only: no game code changed. Inputs: the plan
(`docs/plans/EQOA-MOD-API-PLAN.md`, "Round 7: finish", "Tools for mod authors"), the audit
(`round-7-audit.md`, 219 rows), the spec, GETTING-STARTED, the backlog, and round 6.5 as it stands on
`feat/eqoa-api-r6.5` (b3c11196, not merged). `develop` was 51ba08f5 when this was written.

## What round 7 delivers

1. Every `eqoa.*` and Forge name is real or refuses visibly; the capability list says what the code does.
2. `docs/modding/SUPPORT-TABLE.md` and a machine-readable surface file are printed by the client itself
   (`eqoa mod-api`), and a test fails when they drift from the running client.
3. Author docs a stranger can build a mod from: spec as built, getting started, examples, versioning promise.
4. `eqoa/1` is frozen: a test stops any supported name from disappearing; the owner approves it with the release.

## Findings that shape the plan

- **The audit read source only, on round 6's tip, and its top finding is wrong for real mods.** It says
  `frontiers_forge.icon.GetTexture` always returns nil because `attach_icon_cache` is called only from a test
  helper. `Textures::install_for_addon` (`fork_ext/addons/textures.rs:371`) calls it for every real addon, so
  the stub answers only on a host that has no addon texture service (the same case as the `hud.hidden_by`
  placeholder). The fix is still wanted (the placeholder must refuse, not return nil quietly), but it is small,
  and Part A must reproduce every audit verdict with a test before changing anything.
- **Round 6.5 collides with round 7 in these files:** `fork_ext/addons/native.rs` (the seed table at 65-144,
  `Session`, `install`, `install_fields`), `action_broker.rs`, `native_r6a.rs`, `native_state.rs`,
  `native_tests.rs`, `host.rs`, `runtime.rs`, `manager.rs`, `fork_ext/play.rs`, `docs/modding/EQOA-API-SPEC.md`,
  `GETTING-STARTED.md`, `FORK-UPDATE-NOTES.md`, `docs/fork-footprint.json`, and the bag viewer. Parts A, B and D1
  stay off those lines and can start now. Parts C, D2 and E need round 6.5 in their base.
- **Round 6.5 changes who may act** (manifest `capabilities` now gate `eqoa.actions.*`), and its new names
  (`windows.takeover/release/taken`, `actions.open_loot/talk`, `Entity.npc_flags`, the gate) were not in the
  audit. The freeze must come after 6.5's owner test, and Part C audits those names.
- **The old docs branch is already gone locally.** `docs/mod-author-api` has no branch here; commit 1be52fb is
  kept by the tag `archive/mod-author-api-1be52fb`. What is left is to mine it (Part D), check that the
  `looprook` remote has no such branch, and decide whether the tag stays (question 5).
- **The audit covers functions, not events, record fields or limits.** The spec promises 30+ events and a field
  list per record. Part C checks both by test, so the freeze does not promise things nobody verified.

## Branches and base

- The lead merges `docs/round-7-audit` (the audit and this plan) into `develop` first, so workers have both.
- Round branch `feat/eqoa-api-r7` from `develop`. Part A works on it. Part B works on `feat/eqoa-api-r7b` (new
  files only) and the lead merges it into the round branch. Part D1 works on `docs/eqoa-api-r7-docs`.
- Before Part C starts, the lead brings round 6.5 into the round branch: from `develop` if 6.5 is merged by
  then, otherwise by merging `feat/eqoa-api-r6.5` (round 7 then cannot reach `develop` before 6.5, which is the
  intended order). If 6.5's owner test produces fixes, merge again before the final gate run.
- Footprint base: `reviewed_base` in `docs/fork-footprint.json`. Upstream lines added by round 7: one, in
  `main.rs` (Part B, question 6). Everything else is fork-owned.

## Parts

Builders: **Sol** (Codex; full week from about 03:00 on Wed 11-18; gone Friday 11-20) and **Claude Sonnet**.
Reviewers: **Sonnet** for Sol's work, **Terra** for Sonnet's work, while Terra exists. A fresh Sol session per
part and per fix round. Every brief carries the plan's "Rules for every worker brief". Test names start
`round7<part>_`.

| Part | Work | Builder / reviewer | Size | Needs |
|---|---|---|---|---|
| A | Audit fixes: nothing fake, nothing silent | Sol / Sonnet | Medium | Nothing; starts first |
| B | `eqoa mod-api`, generated SUPPORT-TABLE, drift test | Sol / Sonnet | Medium | Nothing (new files) |
| C | Capabilities from code, round labels, 6.5 and event/field audit | Sol high / Sonnet | Large | A fixed, B merged, 6.5 in base |
| D | Author docs (D1 new pages now, D2 spec and getting started) | Sonnet / Terra | Medium-large | D1 nothing; D2 needs C and 6.5 |
| E | Mod keys against game keys (owner approves the 5 lines first) | Sol, or Sonnet after Friday | Medium | 6.5 in base, question 7 |
| F | `eqoa mod-test` (optional) | Sol if Thursday allows, else Sonnet | Medium-large | B merged, question 8 |
| G | Freeze and release preparation | Sonnet lead, Opus for the release | Small | All of the above, 6.5 owner-passed |

### Part A: audit fixes (Sol; reviewer Sonnet)

Goal: every audit finding is reproduced by a test, then fixed so the name either works or refuses visibly.
The worker brief is `~/Paseo/briefs/eqoa-round-7-partA-brief.md`.

Files (all fork-owned, under `crates/eqoa/src/fork_ext/addons/` unless noted): `icons.rs`, `chat.rs`, `host.rs`
(Forge camera and util only), `package.rs`, `bit.rs`, `file_io.rs`, `fonts.rs`, `map_views.rs`, `visibility.rs`,
`native_items.rs`, `items_forge.rs`, `native_media.rs`; in `native.rs` only the placeholders in `install`
(about 931-964), the arity list (735) and the hand list in `annotate_host` (310-325). Not the seed table, not
`Session`, not `action_broker.rs`, `native_r6a.rs`, `native_state.rs`, the spec or GETTING-STARTED.

Work, in this order:
1. **Reproduce.** One failing test per audit fix (fix list 1-12, the 5 PARTIAL rows, the 2 FAKE SUCCESS rows),
   written before the fix. Where a verdict does not reproduce, record it and skip the fix (GetTexture on a real
   addon host is expected to pass already).
2. **Fake success.** The base placeholders for `frontiers_forge.icon.GetTexture/GetUITexture` (`icons.rs`,
   about 1174-1190) and `eqoa.hud.hidden_by` (`native.rs` 958), plus any other `install` placeholder that returns
   plausible data, return the error contract (`unavailable`, reason "no texture/visibility service on this
   host") with a manager diagnostic. No `eprintln!`.
3. **Partial.** `eqoa.icons.info().source_format` filled from the decoder, or its field described `unsupported`;
   `frontiers_forge.camera.GetCoordinates` with no camera; `frontiers_forge.chat.IsSendHookSuspended`. For
   Forge names the pinned reference decides the return shape; the fix adds the diagnostic or the real value.
4. **Names that crash instead of refusing.** `frontiers_forge.quest` and `frontiers_forge.cave` become guarded
   tables whose members diagnose `unsupported` (so `require` succeeds, spec §11); `frontiers_forge.util` is a
   registered module (visible to `capabilities.list`) and gains `GetWorldId`; `bit.*` gets eight descriptors and
   a diagnosing lookup for other names; `eqoa.fonts.game` is described and either built or refused.
5. **Seeds that never landed.** `eqoa.files.list` built on the existing resolver (cap 1,024, overlay and
   tombstones honoured). `eqoa.map.set_waypoint/clear_waypoint` and `eqoa.map.images/load_image` per question 2;
   until the answer arrives they keep their stub and get an honest reason.
6. **Stale claims.** `MerchantState.button_hint` descriptor, the dead `eqoa.client.time` string, the
   `items_forge.rs` header comment, and an install-order test for `frontiers_forge.player`.

Done means: each fixed row has a test from Play-shaped data or the production install path, with an executed
negative control; **the three bundled UIs and the nine `eqoa_*` mods load with no diagnostic they did not show
before** (a new badge on a bundled mod is a player-visible regression: implement instead of refusing);
`round-7-audit.md` gets a "Part A result" column or section (verdict before, reproduced yes/no, fix commit);
gates `--filter round7a_` then full `--release`; footprint PASS with no upstream line.

### Part B: `eqoa mod-api` and the generated support table (Sol; reviewer Sonnet)

Goal: the client prints its own reference, and the committed table cannot drift from it.

Files: new `crates/eqoa/src/fork_ext/modtools.rs` (command parsing, VM set-up, Markdown and JSON writers) and its
tests; one hook line in `main.rs` `run` (`if let Some(r) = fork_ext::modtools::run(args) { return r; }`), budget
6 to 7 with the reason recorded; `docs/modding/SUPPORT-TABLE.md`, `docs/modding/eqoa-1-surface.json`
(generated); `docs/DEV-TOOLS.md`. No edit to `native.rs` beyond a read accessor if one is missing.

Work:
1. `eqoa mod-api [--format md|json] [--out <file>]`: builds one VM through the production install path (the
   same calls a real `eqoa/1` addon gets, with an addon texture service, no window, no disc, no login), reads
   `eqoa.capabilities.list("")`, prints it. Exit code 0; a broken install prints the error and exits 1.
2. Markdown layout: header (client version, commit, platform, date, "generated, do not edit"), then one table
   per `eqoa` module (path, status, since-round, reason), events, record fields, ImGui/DrawList, legacy aliases;
   Forge per module as a count plus every member that is not `supported`. JSON holds every path.
3. Test `round7b_support_table_matches_the_running_client`: regenerates in-process and compares with both
   committed files; the failure message names the command to rerun. Negative control: remove one `bind`.
4. The table is generated on Windows (the only published platform); the header says so (sound differs elsewhere).

Done means: the command runs from the release build in a terminal; the two files are committed from its output;
the drift test and its negative control are recorded; footprint PASS with `main.rs` at 7.

### Part C: capabilities from code, round labels, the rest of the audit (Sol high; reviewer Sonnet)

Goal: a capability status is a consequence of what is registered, never a separate claim.

Files: `native.rs` (seed, `annotate_host`, `register_descriptor`, `install_fields`, `capability`),
`native_media.rs`, `native_items.rs`, `native_r6a.rs`, `native_events.rs`, `action_broker.rs`,
`window_takeover.rs`, each Forge adapter that holds a stub, tests; regenerated Part B files.

Work:
1. One helper creates a refusing member **and** its descriptor together; every known stub uses it.
   `annotate_host` only describes members nothing else described, and its hand lists (about 300-350) are deleted. A
   function is `supported` because `bind` registered a real function; a seed without a `bind` is `not_yet` or
   `unsupported` with a reason that names when or what replaces it.
2. Module status is computed when asked (`capability`), from its children.
3. **Fields:** for each record type, a test builds the record from a full Play-shaped fixture and checks that
   its keys equal the declared fields, and that every field declared `supported` is non-nil there or sits on a
   short named list of optional fields. **Events:** every `eqoa.events.<name>` described `supported` is emitted
   by a test from Play-shaped data; one that cannot be shown is described `not_yet` and listed in the report
   (report it, do not build it).
4. Round labels follow the plan: 2 core, 3 HUD, 4 map, 5 items, 6 actions/quests/camera/windows/mastery and
   round 6.5's names, 7 for round 7's additions. After the freeze, authors read `revision` (question 4).
5. Audit delta for round 6.5's names with the same verdict table, appended to `round-7-audit.md`.
6. At the end no descriptor says `not_yet` unless its reason names the plan ("after the upstream PR":
   `move`, `look`, `buttons`).

Done means: negative controls for (a) a stub wrongly marked supported, (b) a field removed from a projection,
(c) an event no longer emitted; SUPPORT-TABLE regenerated; the three bundled UIs and all fork mods load
unchanged; zero-addon gate still allocates nothing; gates `--filter round7c_` then full `--release`.

### Part D: author docs (Sonnet; reviewer Terra, before Friday)

D1, new files, can start now (no build, no collision):
- `docs/modding/VERSIONING.md`: the promise in plain words (what never changes under `eqoa/1`, what may be
  added, what `eqoa/2` means, how `eqoa.revision` and `capabilities` are used to feature-test, that Forge
  compatibility is kept but is not part of the promise, how a wrongly supported name gets corrected).
- `docs/modding/EXAMPLES.md`: each bundled `eqoa_*` mod, what it demonstrates, the API areas it uses.
- `docs/modding/template/` (`mod.json`, `main.lua`): a starter that loads clean.
- `docs/modding/AGENT-GUIDE.md`: one page for an agent building a mod from an owner brief.
- `docs/modding/README.md`: the index of the above. Mine 1be52fb (API-CONTRACT, ROADMAP) for anything still true.

D2, after Part C and 6.5:
- Spec as built: new header (no longer "round 1 design"), the hand index in §2 replaced by a link to the
  generated table, §8's opening replaced by the 2026-11-17 rule as 6.5 built it, round numbers in headings and
  §12 renumbered, the "Open questions" addenda folded into their sections, design-time line references
  labelled as such, manifest fields in one table (`entryMode`, `capabilities`, `shares`, `commands`,
  `permissions`, `large_data`).
- GETTING-STARTED: install location, manager, reload, where errors show, capabilities, actions as built.
- `FORGE-COMPATIBILITY-HANDOFF.md` points to the plan and `docs/modding/` (question 5); FORK-UPDATE-NOTES entry.
- One test file `fork_ext/addons/docs_tests.rs`: every fenced `lua` block in GETTING-STARTED and the template
  runs in a fixture host with no error and no `unknown_symbol` diagnostic; every supported function in the
  surface file is named in the spec, and every `eqoa.x.y` the spec names exists. Negative control: misspell one.

Done means: Terra's review is in by Thursday 11-19; the docs test passes with its negative control; no local
paths (`tools/check_no_local_paths.py`).

### Part E: mod keys against game keys (backlog "Round 7 API"; Sol, or Sonnet after Friday)

Today a mod key and a game control can share a key and both fire. Changing that after the freeze would be a
behaviour change, so it belongs before it. Files: `fork_ext/bindpage.rs`, the bind code in `native_state.rs`,
tests. The 5-line description for the owner is under "Owner questions" (question 7). Tests: both directions,
two mods, restart, disable and re-enable, chat and menu suppression; negative control per rule.

### Part F: `eqoa mod-test <folder>` (optional; question 8)

`eqoa mod-test <folder> [--frames N] [--png <file>]`: loads the mod against a built-in saved game state (the
Play-shaped fixture moved out of test-only code), runs N frames, prints errors and diagnostics, exits 1 on any;
writes a screenshot only when a graphics adapter is present. Lives in `fork_ext/modtools.rs`; no further
upstream line. Not needed for the freeze.

### Part G: freeze and release preparation (Sonnet lead; Opus and the owner for the release)

1. All parts and round 6.5 on the round branch; full gates `--release`; footprint and dry-run merge numbers.
2. `eqoa-1-surface.json` becomes the frozen record; test `eqoa1_frozen_surface_only_grows` (no supported path
   or field disappears or drops status; additions allowed) with a negative control. CLAUDE.md gains one rule
   line: `eqoa/1` is additive only, a breaking change is `eqoa/2`.
3. Owner branch build (launcher, settings copy, updater off): the bundled UIs, the fork mods, the bag viewer
   with 6.5, the hearts example and the template mod.
4. Spec header gets the freeze date and version; plan Results entry; backlog items closed or moved.
5. Merge to `develop` on the owner's go-ahead; the Stable release is its own approval (backlog "Final task
   before merging develop into stable").

## Order

| When | Cargo build (one at a time) | Writing and review beside it |
|---|---|---|
| Tue 11-17 evening | none | D1 (Sonnet); owner answers questions 1-8 |
| Wed 11-18, from Sol's reset | A's gates | A builds (Sol); D1 review (Terra) |
| Wed midday | B's gates | B builds (Sol, own worktree) while Sonnet reviews A |
| Wed evening | A's fix gates | A fix round (fresh Sol); Sonnet reviews B; lead merges B and brings in 6.5 |
| Thu 11-19 | C's gates (the long one) | C builds (Sol high); D2 starts on the merged base (Sonnet); E if approved (second Sol; after C's build if C touched `native_state.rs`) |
| Thu evening | C fix, then E or F gates | Sonnet reviews C; Terra reviews D2 and reruns A's and C's negative controls |
| Fri 11-20 morning | last Sol fix round; D2's docs test | Terra's last reviews |
| After Friday | G's full run | Sonnet builds what is left (E or F), reviewed by Hydrogen or free DeepSeek |

Each part runs focused tests (`--filter round7x_`) while working and one full `--release` run at its last
commit (30-40 minutes). The lead reruns the full set once, on the merged branch, in Part G.

## Owner questions

1. **Freeze timing.** The plan says "freeze only after the round 4 mods have shipped". They are on Nightly, not
   on Stable. Freeze with the Stable release that first carries rounds 4 to 7 (recommended), or now on Nightly?
2. **Four map names and one file name promised in round 1 and never built.** Recommended: build
   `eqoa.files.list` (yes/no); build `eqoa.map.set_waypoint/clear_waypoint`, which move the player's own
   waypoint (yes/no; "no" keeps mod-owned guides as the only way); drop `eqoa.map.images/load_image` from
   `eqoa/1` because `eqoa.map.view` replaced them (yes/no).
3. **Wereoxxs in the freeze test.** It is not maintained for now. Is "loads and its tests pass" enough for
   Wereoxxs, with the live check on ARPG UI, Modern UI and the fork's mods (recommended)? Yes/no.
4. **Round numbers in the capability list.** Renumber them to this plan (2 to 7) as the plan says, and tell
   authors to rely on `revision` after the freeze (recommended)? Yes/no.
5. **Old drafts.** OK to (a) delete any `docs/mod-author-api` branch left on the `looprook` remote, keeping the
   archive tag, and (b) rewrite `FORGE-COMPATIBILITY-HANDOFF.md`'s "Next" section to point to the mod API plan
   and `docs/modding/`? Yes/no each.
6. **One line in upstream's `main.rs`** so `eqoa mod-api` (and `mod-test`) exist in the shipped program; the
   lead approves the exact line as in rounds 6 and 6.5. Yes, or no (then the table is produced by a test only).
7. **Mod keys against game keys**: in round 7 with the description below (recommended), or after the freeze?
   If yes, approve or correct the five lines:
   1. On Controls, a key used by both a game control and a mod (or two mods) is marked on both rows.
   2. Giving a mod a key the game uses asks the game's usual question, and the old control is freed or swapped.
   3. A mod's default key never takes a key in use: its row starts empty and says which control has the key.
   4. One press does one thing: never a game action and a mod key together.
   5. This holds after a restart, after disabling and enabling the mod, and from either Controls section.
8. **`eqoa mod-test`**: build it in round 7 if Thursday allows (recommended: only then), or move it to the backlog?

No other part changes what a player sees. Part A may add entries to a mod's error list in Mods & Addons only
for mods that call something unsupported; the bundled mods must show none.

## Risks

1. **The audit is unverified.** One of its two top findings does not hold for real mods. Part A reproduces
   before fixing; expect a few more rows to change verdict.
2. **Round 6.5 moves under us.** Its owner test can change `native.rs`, `action_broker.rs` and the spec after
   Part C has started. Keep C's edits to the seed table in one commit so a re-merge is one conflict.
3. **Round 6.5's permission model becomes contract at the freeze.** A mod whose manifest has no `capabilities`
   list is allowed every action (`has_capability` returns true for `None`). The lead should confirm in 6.5's
   review that this happens only for Forge imports, and that every acting mod is labelled, before the freeze.
4. **Sol ends Friday.** Part C is the one job that needs it. If Sol stops at a limit, build and test its last
   commit before trusting it; Sonnet finishes, and then a non-Claude reviewer is needed (Hydrogen or DeepSeek).
5. **Terra ends Friday too.** Sonnet-built work reviewed after Friday loses its planned reviewer.
6. **New diagnostics on bundled mods.** Turning silent stubs into refusals can put error badges on Wereoxxs,
   ARPG or Modern UI. Part A's "no new diagnostic" check is the guard; the owner's test is the proof.
7. **The spec promises more than functions.** Limits, quotas and diagnostic timing in §2, §8 and §9 are
   design text. Part D2 marks any that Part C's tests did not confirm as "design intent, not part of the
   promise", so the freeze covers names, shapes and meanings that were tested.
8. **Build queue.** About six full gate runs (3 to 4 hours of the PC) between Wednesday and Friday, beside the
   owner's own play tests of 6.5.
9. **The freeze test can block an honest correction** (a supported name found fake later). Policy in
   VERSIONING.md: such a downgrade needs the owner's approval and a player note; editing the surface file is
   the visible act.
10. **ImGui parity and Forge input polling** were not audited per function (audit "UNCLEAR"). They stay covered
    by the bridge tests and the bundled UIs; the support table says so.

## Owner answers (2026-11-17)

All eight questions: the recommended option. (1) Freeze with the Stable release carrying rounds 4-7. (2) Build
`files.list` and `map.set_waypoint/clear_waypoint`; drop `map.images/load_image`. (3) Wereoxxs: loads and tests pass
is enough. (4) Renumber round labels to the plan. (5) Delete the remote `docs/mod-author-api` branch (archive tag
kept) and repoint FORGE-COMPATIBILITY-HANDOFF. (6) One hook line in `main.rs` for `eqoa mod-api`. (7) Key conflicts in
round 7 with the five lines as written (approved). (8) `eqoa mod-test` only if Thursday allows, else backlog.
