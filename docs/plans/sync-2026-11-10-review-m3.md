# EQOA Enhanced upstream sync 2026-11-10 — independent review (round 3)

**Scope:** the hand-resolved files only — `crates/eqoa/src/{hud.rs, play.rs, options.rs,
settings.rs, viewer.rs, bindpage.rs, map/chrome.rs, map/mini.rs}`, `crates/net/src/session.rs`,
and the `fork_ext/**` files changed by `b237316` / `38c930f`.

**Method:** read-only. No build, no cargo, no edits to the branch. Every claim below was
checked by reading the tree at `47a99d1` and comparing it against `5d54c0c` (upstream),
`abd47db` (fork develop) and `91f9e72` (the merge-base). Where I could not confirm a
statement from the code, I say so.

## Verdict

**APPROVE WITH CHANGES**

The merge is, on the whole, careful and faithful: upstream's code survives essentially
intact, upstream's `ALL_CATEGORIES`/`EXTENDED_ITEMS`/page order are byte-identical, every
upstream test function is still present, the `Play` initializer carries all 203 upstream
fields, the loot-roll capture runs before the fork's chat-history tap, and the browser's
`graphics_lost` guard still precedes every native-only hook. The fork's own behaviour
(Stats Box, ToT, Mana Bars, HUD numbers, map search/journal/round minimap, the report
hook, the 64/650 migration, F2 map search) is intact.

Two defects need fixing before this is merged into `develop`/`enhanced`:

* one **duplicated code block** that puts a duplicate line on screen (HIGH), and
* **two upstream test assertions** that upstream restored after the fork's merge-base and
  that the sync dropped instead of taking (MEDIUM).

Both are small and mechanical. Neither is a gameplay-logic regression.

---

## HIGH

### H1 — `crates/eqoa/src/map/chrome.rs:327-338`: the GM-travel legend line is inserted twice

**What is wrong.** `Map::chrome::draw` contains **two identical `if c.travel { … }` blocks**:

```rust
327:     if c.travel {
328:         // a GM's: after the waypoints' keys, a keyboard's and a pad's alike
329:         // (a mouse goes with either)
330:         let at = pieces.iter().rposition(|p| p.1.contains("aypoint")).map_or(pieces.len(), |i| i + 1);
331:         pieces.insert(at, (-1, LEGEND_TRAVEL));
332:     }
333:     if c.travel {
334:         // a GM's: after the waypoints' keys, a keyboard's and a pad's alike
335:         // (a mouse goes with either)
336:         let at = pieces.iter().rposition(|p| p.1.contains("aypoint")).map_or(pieces.len(), |i| i + 1);
337:         pieces.insert(at, (-1, LEGEND_TRAVEL));
338:     }
```

The second block is a byte-for-byte copy of the first, including the comment. This is the
classic "both sides of the conflict won" signature the brief asked me to look for.

**How I verified it.**
- `git show 5d54c0c:crates/eqoa/src/map/chrome.rs` has **one** `if c.travel` block.
- `git show abd47db:crates/eqoa/src/map/chrome.rs` (fork develop, pre-sync) has **one**.
- `git show 47a99d1:crates/eqoa/src/map/chrome.rs` (HEAD) has **two**.
- The duplication is present from `724e7fa` (batch 1) onward and survives `ef3c265`,
  `b237316`, `bb04abe` and `38c930f` — i.e. it was introduced in the *first* upstream
  merge and was never caught.
- A whole-file duplicate scan (all 9 hand-resolved files, blocks of 3–12 identical
  consecutive lines) reports this as the **only** duplicate in the set.
- Upstream's own test only asserts *presence*:
  `assert!(r.0.iter().any(|s| s == LEGEND_TRAVEL), "{:?}", r.0);`
  (upstream's `the_chrome_says_where_it_goes`, `crates/eqoa/src/map/chrome.rs:725`), so
  the duplicate passes the suite.

**Failure scenario.** A GM on go-eqoa-server (the only server that offers `OP_MAP_TRAVEL`,
`crates/net/src/session.rs`) opens the map. `Chrome.travel` is true, so the legend gets
"Ctrl+click Travel here" (or "Cmd+click Travel here" on macOS,
`crates/eqoa/src/map/chrome.rs:29-32`) inserted **twice**, back to back. The legend is
laid out in wrapped lines by `wrap_keyboard_label` (`chrome.rs:344`), so the player sees
the same hint repeated, wasting a legend line and making the map's help text look
broken. It is visible on every map open for a GM — a player-visible defect in the exact
feature the batch-2 report claims to have kept ("GM travel legend, inserted after
waypoints").

**Fix.** Delete lines 333–338 (the second block). Keep the first.

**Note on the report.** `docs/plans/sync-2026-11-10-report.md:19` says for
`map/chrome.rs`: "Kept from upstream: GM travel legend, inserted after waypoints." The
legend was kept — twice. The report does not mention the duplication, and the
"no upstream code or tests lost" claim in the Batch-2 table is therefore not accurate for
this file.

---

## MEDIUM

### M1 — `crates/net/src/session.rs:3492` and `:3767`: two upstream assertions dropped instead of merged

**What is wrong.** Upstream `5d54c0c` contains two assertions that HEAD does not:

```rust
// crates/net/src/session.rs, quest-marks test (upstream 5d54c0c:3336-3337)
s.agree_quest_marks(false);
assert_eq!(answers(&mut s), [], "the Quest Helper off from the start: nothing answered, as today's");

// crates/net/src/session.rs, quest-progress test (upstream 5d54c0c:3611-3612)
s.agree_quest_progress(false);
assert_eq!(answers(&mut s), [], "the Quest Helper off from the start: nothing answered");
```

At HEAD the corresponding regions read (session.rs:3492-3495 and :3767-3770):

```rust
deliver(&mut s, drdp::TYPE_GUARANTEED, vec![0x03, 0x7E, 1, 0, 0, 0]);
s.agree_quest_marks(true);
s.agree_quest_marks(true);
assert_eq!(answers(&mut s), [(251, vec![0x03, 0x7E, 1, 0, 0, 0])], "asked once");
```

i.e. the "off from the start" case is simply gone.

**How I verified it.**
- `git diff 5d54c0c 47a99d1 -- crates/net/src/session.rs` shows exactly these four lines
  (`-`) and nothing added in their place.
- A line-by-line assertion diff over all 9 hand-resolved files confirms these are the
  **only** two upstream `assert…` lines in the whole set that are absent from HEAD.
- `git log -S "the Quest Helper off from the start: nothing answered"` finds upstream
  commit **`65f9b13` "The map and the quest helper back (Extended > Map, on by default)"**
  (Winfro/DabDavis, 2026-11-09), which is an ancestor of `5d54c0c` and therefore **upstream
  code**, not fork code. `git merge-base --is-ancestor 65f9b13 5d54c0c` → exit 0.
- The fork's develop (`abd47db`) predates that upstream restoration (its merge-base is
  `91f9e72`), so the fork side of the conflict legitimately lacked these lines and the
  merge resolution kept the fork's version instead of taking upstream's.

**Failure scenario.** The upstream assertion guards a real client behaviour: a client
that has the Quest Helper **off** must not answer the server's `OP_QUEST_MARKERS` /
`OP_QUEST_PROGRESS` offer with a "0" it never asked for. Upstream's `agree_quest_marks`
satisfies this only because of the `unwrap_or(false) == want` early return. The fork's
implementation at HEAD (`crates/net/src/session.rs:1364-1376`, upstream's own, unchanged)
happens to satisfy it too, so nothing is broken today — but the coverage that upstream
added on 2026-11-09 is gone, and the next change to `agree_quest_marks` (for instance the
fork's own `agree_power_bars`, which uses the *other* form
`self.power_bars_asked == Some(want)` and **would** answer "0" spuriously) would not be
caught by any test.

**Fix.** Restore both two-line blocks from upstream. They pass with the current
implementation (`None.unwrap_or(false) == false` → early return → nothing sent), so this is
a pure coverage restoration, not a behaviour change.

**Note on the report.** `docs/plans/sync-2026-11-10-report.md:215` says for
`net/session.rs`: "No upstream test/assertion was removed." That is not accurate: two
were. The report's separate claim (line 38-40) about the *inherited* absence of
`the_servers_power_offer_goes_unanswered` is accurate and correctly labelled — that one is
a genuine fork behaviour change (the fork answers the power offer for Mana Bars) and is
the **only** upstream test *function* missing from the nine files.

---

## LOW

### L1 — `crates/eqoa/src/bindpage.rs:640-641`: upstream's "Hide UI: F2" assertion rewritten to "F10"

**What is wrong.** Upstream's `what_cannot_move_and_defaults` asserts
`Some("Hide UI: F2. H does nothing now.")`; HEAD asserts
`Some("Hide UI: F10. H does nothing now.")` (`bindpage.rs:640-641`).

**Why it is not a regression.** This is a pre-existing fork adaptation, not something this
sync introduced: `git diff <merge-base> abd47db -- crates/eqoa/src/bindpage.rs` shows the
identical `f2` → `f10` change already in develop. F2 is the fork's `map_search` default
(`crates/eqoa/src/binds.rs:166-172`), so upstream's "capture a free key" step would
otherwise swap `map_search` and change what the assertion means. F10 is free by default,
so the assertion keeps its shape and its meaning ("a row moved onto a free key says its
old key does nothing now").

**Failure scenario.** None in the product. The loss is coverage symmetry: the *other*
upstream test that needs F2 free (`a_row_takes_its_key_and_button_back_from_a_macro`) was
handled properly by the new `fork_ext::bindpage::free_fixture_key` hook
(`crates/eqoa/src/fork_ext/bindpage.rs`), which frees F2 only inside that fixture and
proves production keeps F2 (`fork_ext/bindpage.rs:13-23`). The same hook could have been
used in `what_cannot_move_and_defaults` — but that test reads `settings::binds()` (the
live store) rather than a local `Binds`, so it would need extra setup. Applying the hook
consistently to both would let upstream's assertion text stay verbatim.

**Suggestion.** For symmetry with the F2 fixture hook, either (a) apply
`free_fixture_key` to the store's binds in `what_cannot_move_and_defaults` and restore
upstream's `"Hide UI: F2. H does nothing now."` text, or (b) record the substitution
explicitly in the footprint's `reason` for `crates/eqoa/src/bindpage.rs`, which currently
mentions only the `'y'` macro-key substitution.

### L2 — `crates/eqoa/src/play.rs:9980-9985`: upstream's `hidden_the_huds_buttons_take_no_click` replaced, not extended

**What is wrong.** Upstream's version of this test asserts that a click on the quick
menu's title bar turns to the next menu (`assert_eq!(p.hud_menus.sel, 0)` /
`assert_ne!(p.hud_menus.sel, 0)`). HEAD's version tests the compass instead
(`crates/eqoa/src/play.rs:9980-9985`): the compass click opens the map, and the map is
not opened while the UI is hidden. The four upstream assertion lines are gone (they are
the only upstream `assert` lines missing from `play.rs`).

**Why it is not a regression.** Pre-existing in develop — `git diff <merge-base> abd47db
-- crates/eqoa/src/play.rs` shows the identical replacement. It is a fork behaviour (the
fork binds the map to the compass) that needed a different observable, and the fork's
`hud_menu_visible` claim hook means the title-bar click is now addon-claim dependent.

**Failure scenario.** No product failure. The loss is that upstream's quick-menu
title-bar click path is no longer covered by any test in the fork; a regression in
`HudMenus::point` / `hud_menus.sel` would go unnoticed.

**Suggestion.** Keep the fork's test and add upstream's assertions back as a second case
in the same test (or a sibling test), so both observables are covered.

### L3 — `crates/eqoa/src/map/chrome.rs:660-661, 668-669`: upstream test lines reformatted

Upstream's `the_chrome_says_where_it_goes` and `the_notice_goes_under_the_title` had
single-line closures that HEAD has wrapped across several lines (rustfmt-style). The
assertions themselves are unchanged and all present. Strictly, "never reformat upstream
lines" is violated for four cosmetic lines. Cosmetic only; no behavioural impact, and it
is pre-existing in develop. Flagged for completeness only.

### L4 — `crates/eqoa/src/map/chrome.rs` / `map/mini.rs`: the fork's map redesign sits in upstream files

The fork's parser-inspired surfaces (`super::theme::panel`, `super::theme::TEXT`), the
circular minimap mask (`Mini.circle`, `fork_ext::map::circular_rim`), the ring border and
the rebound keyboard legend all live in upstream-owned files (`map/chrome.rs`,
`map/mini.rs`). The footprint accounts for this: `docs/fork-footprint.json` lists the map
family as *shared* with measured budgets, and the report notes the circle geometry was
moved to `fork_ext/map.rs`. The placement is therefore deliberate and budgeted, not a
merge artefact.

Residual risk worth a follow-up (not a defect in this sync): the circular minimap mask
replaces upstream's rectangular `mini_contains` (`crates/eqoa/src/map/mini.rs:433-437`),
so the four rectangle *corners* of the minimap are now outside the clickable region.
That is the fork's design and its own tests cover it
(`map/mini.rs` `diagonal_guidance_uses_a_circle_rim_and_corners_are_outside`).

---

## What I checked and found correct

These are the specific concerns from the brief; each was verified in the code.

**Upstream code and tests preserved.** Every upstream test *function* in the nine files is
present at HEAD. The only upstream test function missing is
`the_servers_power_offer_goes_unanswered` (`crates/net/src/session.rs`), replaced by the
fork's `power_bars_offer_answer_and_set` — a genuine behaviour change (the fork answers
the power offer for Mana Bars) and correctly documented in the report at lines 38-40.
`#[test]` counts never decrease: hud 9→11, play 82→86, options 20→20, settings 46→46,
viewer 4→4, bindpage 7→7, map/chrome 6→11, map/mini 9→14, net/session 45→47.
Upstream's `the_raid_windows_are_agreed_read_and_sent` and
`map_travel_offered_then_asked` are byte-identical to `5d54c0c` (compared line by line).

**Options.** Upstream's `ALL_CATEGORIES` (12 pages) and `EXTENDED_ITEMS` (58 rows) are
byte-identical to `5d54c0c`, in the same order. The fork's two pages are appended
*after* upstream's last page ("Combat"): `fork_ext/options.rs` `categories()` builds
`[upstream…, HUD_CATEGORY, CATEGORY]`, native 12→14, wasm 11→13 (wasm drops Textures,
matching upstream's own wasm `CATEGORIES`). The runtime reads `fork_options::pages()`
everywhere (`options.rs:657-658, 963-967, 1010, 1269, 1279, 1317`), and every remaining
use of the upstream `CATEGORIES` const in `options.rs` is inside `#[cfg(test)]`. The
`change` hook was placed in the **inner** `_ => {}` arm of upstream's
`item if extended(item)` block (`options.rs`, inside `fn change`), with upstream's outer
`_ => {}` left untouched — the fork's rows reach it because `extended()` now consults
`fork_options::items()` (upstream's rows chained with the fork's). Upstream's
`CATEGORY_ROWS`/`CATEGORY_LEGEND` consts are kept (with `#[allow(dead_code)]` on the
former) rather than deleted.

**Play.** The merged `Play { … }` initializer carries **all 203** upstream fields; the
only two additions are `forge_health` (cfg-gated) and `trainer_from_hud`. Field-wise
merging worked. `self.session.agree_power_bars(crate::settings::client().mana_bars)` is
called in the same per-frame block as upstream's `agree_pet_effects` /
`agree_number_tags` / `agree_dps_meter` (`play.rs:3477`).

**Key-handling order.** `Play::chat_scroll_key` (`play.rs:4110-4120`) runs
`self.roll_key(key, down)` **first** and returns when the loot roll owns the key, exactly
as upstream `5d54c0c` does; the fork's `crate::hud::fork_hud::chat_history_key(key,
down)` tap runs *after* that check and *before* `wnd_chat.key_down`. The tap is
non-consuming (`fork_ext/chat_history.rs:55-71` returns `()` and only counts scroll
steps), so it cannot steal the roll's keys or the chat window's. `fork_ext/chat_history.rs`
is unchanged by this sync.

**Browser vs native cfg.** In `viewer.rs`, upstream's
`#[cfg(target_arch = "wasm32")] if crate::web::graphics_lost() { el.exit(); return; }`
still runs *before* the fork's `#[cfg(not(target_arch = "wasm32"))] fork_report::event(…)`
and the `forge_runtime.event(…)` hook (`viewer.rs`, `fn window_event`). The report's
claim that the two are mutually exclusive holds. The one non-cfg-gated fork call,
`crate::report::is_open_local()` in `finger_on` (`viewer.rs:3129`), is safe: that function
returns `false` on wasm (`fork_ext/report_core.rs:2270-2275`). `crate::report::draw`,
`is_open_local` and `report_core::open` are all wasm-safe or wasm-disabled
(`report_policy::enabled()` returns `false` on wasm, so the HUD report button is not even
drawn there).

**F2 map search.** The fork binds `map_search` to `f2` by default
(`crates/eqoa/src/binds.rs:166-172`) and upstream's `bindpage.rs` macro fixture needs F2
free. The resolution is clean: `fork_ext/bindpage::free_fixture_key` (test-only module,
`fork_ext/mod.rs:45-46`) moves only that fixture's `map_search` to `f24`, and its own test
proves `Binds::default()` still has `map_search` on `f2` in production. Upstream's
header comment in `bindpage.rs` was restored verbatim. Upstream's
`the_extended_pages`, `each_category_opens_its_rows_and_back_goes_up_a_step`,
`every_row_on_one_page_and_no_page_scrolls` and `a_pointer_on_the_extended_pages` are
unchanged and still use upstream's own tables.

**Settings and saved keys.** `settings.rs`'s fork footprint is +51/-1 (was +52/-1 before
the sync) and consists only of: the `fork_ext/settings` module declaration, the
`report::update_saved_login_cache` hook, the three fork `ClientOptions` fields plus their
defaults, the `fork_settings::write`/`parse` hooks, the `migrate_defaults` call in
`read_shared`, `set_stats_box`/`stats_box`, and one test setup line. Upstream's
`read_shared` body is otherwise unchanged; `agree_quest_marks`/`agree_quest_progress`
implementations in `session.rs` are upstream's own. The fork's rows are written and
parsed by `fork_ext/settings.rs:117-159` (`target_of_target`, `mana_bars`, `stats_box`,
`target_hp_percent`, `xp_details`, `hud_numbers`), and the `enhanced_rows()` guard
(`fork_ext/settings.rs:107-116`) keeps upstream's own fixtures on upstream's rows, so
upstream's `options_with_the_lines_of_rows_gone_still_load` still asserts what it did.
The 64/650 migration (`fork_ext/settings.rs:12-84`) is unchanged by the sync and still
writes `entity_slots=64` / `entity_view=650` only when the file has no explicit in-range
value, and still leaves an explicit 24/100 alone.

**HUD.** Upstream's DPS-meter state/picks/draw (`hud.rs:498-527`), the quest-tracker's
room, the pet-effects strip and all their tests are intact. Upstream's
`a_pointer_finds_the_minimap_where_the_quick_menus_go` was relocated (not deleted) and
still asserts the same three window sizes; the fork's new assertions were added around
it. The ToT bar, Mana Bars, Stats Box and numeric readouts are the fork's, unchanged from
develop, and `fork_hud::draw` (`fork_ext/hud.rs:259-282`) draws only the numeric
readouts — no double-draw against `hud.rs`.

**Map.** Upstream's `to_rim` rectangle helper, `place_marks`, `MarkKind::QuestOffer` /
`QuestStep`, `on_rim`, the GM-travel `Chrome.travel` field and `LEGEND_TRAVEL` are all
present; the fork's circular rim is a separate helper in `fork_ext/map.rs` applied
*after* upstream's placement (`map/mini.rs:559`), so upstream's
`the_quests_steps_point_the_way_from_the_rim` still tests upstream's placement contract
verbatim. The upstream GM-travel `map_travel_to` / `OP_MAP_TRAVEL` protocol is upstream's
now; the fork's duplicate method and duplicate test were correctly removed from
`fork_ext/play.rs`, and upstream's `map_travel_offered_then_asked` remains.

**No duplicated logic elsewhere.** A duplicate-block scan over all nine files (3–12
identical consecutive lines) reports only H1. No "both sides running" pattern was found in
`hud.rs`, `play.rs`, `options.rs`, `settings.rs`, `viewer.rs`, `bindpage.rs`,
`map/mini.rs` or `session.rs`.

## Not verified

- I did not build, run cargo, or execute any test. All behavioural statements are from
  reading the source.
- I did not review the non-hand-resolved parts of the merge (the ~85 automatically merged
  upstream files, the new raid modules, `web/`, `tools/`, the release/packaging scripts).
- I did not verify the report's gate results (test counts, GPU failures, footprint
  metrics, packaging receipts); I only checked that the *code* matches the report's
  descriptions where I could.
- I did not verify the physical-device, live-browser, live-server or Forge/golden
  fixtures the report lists as outstanding.
- The claim in the report that the two GPU failures reproduce on untouched upstream is
  outside my scope and I take no position on it.
