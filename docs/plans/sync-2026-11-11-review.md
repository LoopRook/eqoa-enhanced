# Review: sync/2026-11-11 (upstream a3cedec merged into develop cbcf86b)

Reviewer: independent, read-only (git and file reads only; no cargo run). Scope: the seven hand-resolved
files, the new crash-label hook, and the changed `fork_ext/` files. Compared against `a3cedec` (what the
fork adds over upstream) and `cbcf86b` (what changed for the fork).

## Verdict: APPROVE WITH CHANGES

The hand resolutions are sound. Every upstream behaviour I checked is intact (indoor minimap, places list /
P / Where I am, crash sender 0x7E64-66, Q-tap mob info, item links, merc name colours, Hide Look). The fork
features are still wired (round mask and hover cards, F2/F4, journal recording, binding-aware legend, local
crash offer, addon hooks). No upstream test was removed or weakened by this sync, apart from inherited
differences. One HIGH defect comes from the new upstream crash feature meeting the fork's local crash
reports, and it should be fixed before this merges to `develop`. The Support-petition gap Sol disclosed is a
MEDIUM against the "every report is labelled Enhanced" rule.

## HIGH

### H1. Upstream's kept crash reports and the fork's local crash reports share one folder and one file pattern

Files: `crates/eqoa/src/crashfile.rs:132` (`dir()` = `<config>/crashes`) and `:136` (`files_in` takes every
`crash-*.txt`); `crates/eqoa/src/fork_ext/report_core.rs:480-481` (`crashes()` = `<config>/crashes`), `:509-515`
(writes `crash-<run>.txt`), `:621-628` (`waiting_in` takes every `*.txt`);
`crates/eqoa/src/logfile.rs:131,133` (the panic hook calls upstream's `keep_crash` and then the fork's
`crate::report::crash`). `crashfile.rs` is new in this merge (absent in `cbcf86b`), so this collision is
introduced by the sync, and the report does not mention it.

Failure scenarios:

1. **One panic, two reports.** A panic writes `crash-<stamp>-<secs>.txt` (upstream) and `crash-<run>.txt`
   (fork). The local "Save to Desktop" offer (`offer_in`, `report.rs:~660` `Kind::Crash(files)`) lists both,
   so the zip holds the same crash twice and the dialog says "a crash report each time" twice. On a
   supporting server `crashfile::pending()` sends both, using two of the three per-login slots (`SENT_A_LOGIN`).
2. **The fork's richer local file goes to a game server.** `files_in` matches the fork's
   `crash-<run>.txt`, whose body is "EQOA native client crash report / version / [client, as last noted] /
   [log] last N lines" (`report_core.rs:519`). `send_pending` prefixes the Enhanced label and sends up to
   12,000 bytes of it, so the player's redacted log tail and state snapshot reach whichever server offered
   crash reports. That goes beyond upstream's scrubbed report and contradicts FORK.md's "Nothing is
   uploaded" for local reports. The same happens to the file `take_last_run` writes for a run that never
   closed.
3. **Each side deletes the other's file.** A server acknowledgement (`crashfile::answered` -> remove file)
   deletes a report before the local offer shows it. Saving locally (`report.rs:838` `remove_file`) deletes
   a report the server was about to get. The pruning also crosses: `write_in` keeps `KEPT=5` over both
   kinds, and `report_core::start` (`CRASHES_KEPT=5`, `:335`) prunes both kinds too, so a burst of one kind
   evicts the other.
4. The server-side report format becomes unpredictable (two different headers: "EQOA client crash report"
   and "EQOA native client crash report").

Fix (stays in fork-owned code, no upstream edit): move the fork's local crash folder to its own name (for
example `report-crashes/`) in `report_core.rs` (`crashes()`, the zip paths and the offer), and at start move
any legacy `crash-*.txt` whose first line is "EQOA native client crash report" out of `crashes/` into it,
so players who already have waiting files do not suddenly send them to a server. Add a test: write one
fork-style file and one upstream-style file into the right folders; `crashfile::pending_in` returns only the
upstream one, `report_core::waiting_in` only the fork one, and `answered` on one does not remove the other
(the negative control is the current shared layout, where all three fail).

## MEDIUM

### M1. F3 Support petitions and the bug note that precedes them carry no Enhanced label

Files: `crates/eqoa/src/play.rs:5512-5519` (`send_bug_note`), `:5544` (`bugnote::compose`),
`crates/eqoa/src/bugnote.rs:54-57,130-131` (`build()` is `dev-<sha>`, `page <b>` or `local`),
`crates/eqoa/src/support.rs:256`. The sync report discloses this ("F3's server Support petitions do not
carry an Enhanced label"), and it pre-dates this sync. It still breaks the owner's stated rule (memory
`reports-enhanced-only`; CLAUDE.md "Reports") and the task's "every report this build sends to a server".
Scenario: a player on a supporting server files F3 -> Support; the GM or upstream maintainer reads
`build: dev-1a2b3c4` and a fork bug looks like a vanilla one. The crash path is now labelled, so the two
server paths are inconsistent.
Fix: one fork-owned hook at the bug note (the note is composed in play.rs; the `fork_ext::play` adapter
already exists) that prepends `enhanced_label()` as the note's first line within `BUG_NOTE_MAX`, with a test
and negative control; or record the owner's decision to leave petitions unlabelled. It is cheap to do now
beside the crash hook.

### M2. A server limit just above the label's length consumes the report with almost no body

File: `crates/eqoa/src/fork_ext/report_label.rs:~28-31`. The check is `min(most, 12000) < label.len()`. With
`label.len() <= most < label.len() + ~100` the report is sent as the label plus a few bytes of body, the
server acknowledges it, and `crashfile::answered` deletes the file. The crash is lost, and so the "stays
kept" guarantee does not hold. A real server offers 12,000 or more, so the case is unlikely today, but the
rule should be "label + newline + a useful minimum (say 512 bytes of body)", else send nothing and keep.
The test "whole identity survives wire truncation" calls `pending` with `most == label.len()` and asserts
only on the returned string, never through `Session::send_crash_report`, so it does not prove the claim.

## LOW

- L1 `fork_ext/report_label.rs`: when the label does not fit, `send_pending` returns 0 and logs nothing
  (`crashfile.rs:250-258` logs only when something was sent). Add one `eprintln!` so a withheld report is
  visible in `eqoa.log`.
- L2 `report_label.rs` tests: the `!body.starts_with("EQOA Enhanced")` "unlabelled negative control" is
  trivially true and tests nothing. No test drives `crashfile::send_pending` itself, so deleting the
  one-line hook is caught only by the footprint hook count (`docs/fork-footprint.json`, "Enhanced crash send
  identity"). A loopback `Session` test through `send_pending` would close this. No double-labelling exists:
  neither `keep_crash`, `compose`, `from_last_crash` nor the fork's files begin with the label, and the label
  is applied to a copy, never stored.
- L3 `viewer.rs` (inherited from earlier syncs, still present): upstream comment lines were rewritten (the N
  and F key comments near `:4370`) and `p.chat_let_go()` was reordered after `p.map.let_go()` /
  `mini_hover(None)` (`:5362`). Harmless, but it edits upstream lines in a shared file against FORK.md's
  "never rewrite"; restore the upstream text on a convenient sync.
- L4 `crates/eqoa/src/play.rs:12163-12165` has a stray doc comment ("What shows the UI again...") glued above
  `the_info_window_says_a_race_label` (upstream has it once, above `a_window_up_shows_the_ui_again`; the
  fork now has it twice). `crates/net/src/session.rs:4467-4477` repeats the "Quest marks" and "quest
  tracker's offer" comment blocks with no test under them (also in `cbcf86b`). Dead comments from earlier
  merges.
- L5 `map/chrome.rs:~283` keeps the upstream `pieces.insert(at, ...)` line but re-indented and altered
  (`if c.keyboard {(-1,"J Quests")} else {(215,"Quests")}` became `(215,"Quests")`), then replaces the keyboard
  `pieces` wholesale from `fork_ext::map::keyboard_pieces`. Correct today (the keyboard path never uses that
  line), but a future upstream change to the keyboard legend will be silently ignored: the replacement derives
  from `KEY_LEGEND`, which is shared, so upstream edits to that const still flow; edits to the inline pieces
  do not.
- L6 With the places list open, F2 still opens the search toolbar behind it (`viewer.rs` F2 handler checks
  only `!keyboard_typing`), and both draw. Keys then go to the search only; the list is not dismissed.
  Cosmetic.
- L7 `docs/fork-footprint.json` `review_state` text was not updated for this sync (only `reviewed_base` and
  the crashfile budget were).
- L8 Rebinding "Quests" (`look_left`) to F4 is allowed by the legend tests, but `viewer.rs` swallows F4 for
  `search.stop()` in plain play before the map sees it. Edge case; the legend would advertise a dead key.

## Checks by item

1. Upstream behaviour kept. Compared `git diff a3cedec HEAD` per file. Deleted upstream lines are limited to
   fork replacements that predate the sync (search fields, Hud picks, theme calls), plus upstream tests'
   fixture lines noted below. Confirmed present: `indoor`/`places` in `map/mod.rs`, `Map::keyboard` P/Start
   -> `toggle_places`, `take_place`, "Where I am" row, `indoor_probe` and `indoor_draw` in viewer, picture
   quad drawn inside the masked layer (`mini.rs` `indoors`/`picture`), `OP_CRASH_*` 0x7E64-66 and the 12k/UTF-8
   cut in `session.rs`, `Q` tap (`info_let_go`), `ChatLinks`/`event_no` sequence, `open_chat_line` (native
   chat gate kept only at the click site `play.rs:5033`, as before), `name_color` with `MERC_COLOR` feeding
   plates and `Dot::of`, `hidelook`.
2. Fork features wired: circle mask (`mini.circle`, `masked_until`, `draw_circle`), hover cards (`dot_under`),
   F2/F4 and search routing in `viewer.rs`, `journal::record_entities` (`play.rs:8769`), binding-aware legend,
   local crash offer (`crate::report::crash` kept in the hook), addon overlay beside the indoor probe.
3. Crash label hook: `crashfile.rs:251` is the single choke point (`send_crash_report` has no other caller;
   native, browser and legacy page fallback all pass through `send_pending`). Label applied to a copy; not
   fitted -> nothing sent, files kept; ASCII label, so the UTF-8 cut in `send_crash_report` is unaffected.
   Findings: H1, M2, L1, L2.
4. Duplicates and key routing: one GM travel legend entry on the keyboard path (`keyboard_pieces` replaces,
   upstream's insert applies to pad/touch only); `P` is reachable only with the map open (`map/mod.rs:2171`),
   F2/F4 are viewer-level and do not collide with it (L6, L8). No duplicate `fn`, `mod` or test name found;
   only stale comments (L4).
5. Tests: only upstream test function missing versus `a3cedec` is `the_servers_power_offer_goes_unanswered`
   (replaced by the fork's `power_bars_offer_answer_and_set`; inherited, the fork implements Mana Bars).
   Fixtures changed by earlier rounds, not this sync: Hide UI quick-menu test now clicks the compass, entity-view
   fixture adds `entity_view`. The 16 new upstream tests are verbatim. Fork tests I read have real assertions
   (`keyboard_hints_follow_the_controls_bindings`, circle/hover tests, `master_off_releases_...`,
   the indoor mask test with its unmasked control). Gaps: the two GPU tests SKIP without an adapter (CI
   passed, per the report); the indoor test draws a render-target quad directly, not through `mini_frame`.

Not verified (no cargo run, no live play): any claim of passing tests; GPU output; the native indoor scene.
