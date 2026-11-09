# Sync 2026-11-09 — independent code review

Reviewed: merge `3307f50` (upstream `91f9e72`, 134 commits) + follow-up `ee7ffa6` + report `4ab2fef`.
Base: our fork `4ebe0f7`. Worktree: `sync/2026-11-09`. Read-only review; no code, cargo or git state changed.

## Verdict

**APPROVE WITH CHANGES**

The merge is technically sound. Every upstream file is present (471/471), the fork's API
surface is untouched, the GM-travel protocol and the quieter quest rim are byte-faithful
ports of upstream `476fdac`, the option-ID space has no collisions, every saved-config key
survives, and the two AO failures genuinely cannot be caused by our changes (the AO code,
shaders and tests are byte-identical to `91f9e72`).

The changes required are all in the *test* and *hygiene* layer, plus one dead-code block:
one test was silently dropped, one test's assertions were weakened to `"(null)"`
placeholders, and the merge left a duplicated, unreachable key-handling block in the
viewer. None of these affects shipped behaviour; all of them should be fixed before the
branch merges into `develop`.

---

## HIGH

### H1 — The options-menu save round-trip test was dropped by the merge

`crates/eqoa/src/options.rs` — the test `the_extended_pages` now ends at line 2068 with
`assert_eq!((w.cat_sel, cues.last()), (11, Some(&9)), "Textures, the last category");`.
Upstream `91f9e72` continued that same test with the save round-trip
(`91f9e72:crates/eqoa/src/options.rs:2116-2167`):

```rust
assert_eq!(testing::kept("client.opt"), None, "nothing kept yet");
// ... walk to Save Changes, Yes ...
assert_eq!(w.save_count, 3, "Saving... (one frame drawn)");
let kept = String::from_utf8(testing::kept("client.opt").unwrap()).unwrap();
assert_eq!(kept, "lod=on\nbrightness=4\ncamera_distance=11\ncamera_turn=125\nfog=4\n");
assert_eq!(GameOptions::from_card(&testing::kept("game.opt").unwrap()).unwrap().auto_attack, 1);
```

Our fork had the same block (`4ebe0f7:crates/eqoa/src/options.rs:1872,1911,1915`).

Failure scenario: the merge rewrote the Extended page walk so the categories are looked up
by name and the page order changed (12 categories now, `Textures` at index 11). If a
future edit breaks the Save Changes path — the menu's `sel` no longer reaching the
"Save Changes" row, the confirm box's "Yes" not committing, or `set_client(c)` not
persisting the row the user just moved — **no test in the workspace fails**, because
`settings.rs`'s own tests exercise `settings::save()` directly and never drive the
options menu. The `client.opt` / `game.opt` write path from the menu is now untested.
This is the one place where the "Save Changes" round-trip was covered end to end.

Fix: restore the block, re-walking the new category order. `w.sel` is now 5 (Game,
Interface, Sound, Extended, Controls, Save Changes) — the upstream key sequence
`DOWN, DOWN, CROSS, LEFT, CROSS` still reaches Save Changes + Yes, but the intermediate
"Video / World / Camera" sub-walks must be re-checked against the new 12-category order
(`Video`=0, `World`=1, `Camera`=5).

### H2 — The viewer's map key-capture block is duplicated, the second copy is dead

`crates/eqoa/src/viewer.rs:3647-3661` and `crates/eqoa/src/viewer.rs:3673-3686`.

```rust
3647:  // The map (`map.rs`), while it is up, takes every key.
3648:  if let Some(p) = self.play.as_mut().filter(|p| p.map.open) {
...      // sets p.keyboard = true
3660:      return;
3661:  }
3662:  // F3: a bug report, Support's request form on Bug Report ...
3673:  // The map (`map.rs`), while it is up, takes every key.
3674:  if let Some(p) = self.play.as_mut().filter(|p| p.map.open) {
...      // does NOT set p.keyboard = true
3685:      return;
3686:  }
```

The first block returns unconditionally when the map is up, so the second is
unreachable. This is the merge's conflict resolution: the fork already had a map
key-capture block (`4ebe0f7:crates/eqoa/src/viewer.rs:3509`, which sets
`p.keyboard = true`), and upstream added its own (`91f9e72:.../viewer.rs:3585`, which
does not). The merge kept both.

Failure scenario: none today — the surviving first block sets `p.keyboard = true` and
dispatches identically — but the dead copy is a live trap. The next person to "fix" the
F3 ordering (see H3) will edit the wrong copy, and the second copy is the one upstream
wrote and the one whose `p.keyboard = true` omission upstream never needed.

Fix: delete lines 3673-3686.

### H3 — F3 (the bug report) is dead while the map is up; upstream ordered F3 first

`crates/eqoa/src/viewer.rs:3647` (the surviving map block) returns before the F3 handler
at `crates/eqoa/src/viewer.rs:3662-3672` is reached.

Upstream deliberately ordered **F9 → F3 → Map** (`91f9e72:crates/eqoa/src/viewer.rs:3569-3598`)
so that F3 opens the Support form even with the map up. `Play::open_bug_report`
(`91f9e72:crates/eqoa/src/support.rs:92`) only requires `self.mode == 0`, and the map is
the client's own overlay — it does **not** change `mode`. So upstream's F3 handler did
fire with the map up.

In the merged tree, F3 with the map up is routed into
`p.map.keyboard_key("f3", …)` → `map::keyboard("f3")` → `None`
(`crates/eqoa/src/map/mod.rs:1835-1852` has no `"f3"` arm) → nothing happens.

Failure scenario: a player on a server that offers bug reports (`Session::bug_reports`)
opens the map to look up a location, presses F3 to file a bug about what they are
looking at, and nothing happens — no form, no cue, and (because the map block consumed
the event) no error either.

Note on attribution: this ordering was **already** the fork's before this merge — the
fork's `4ebe0f7` has **F9 → Map → F3** (`4ebe0f7:crates/eqoa/src/viewer.rs:3504-3534`).
So the merge did not introduce the regression; it preserved the fork's (worse) order
while adding upstream's block as dead code. It is reported here because upstream's
ordering was one of the "upstream behaviour to keep" that this sync was supposed to
reconcile, and the report (`docs/plans/sync-2026-11-09-report.md:29`) claims viewer.rs
retains "upstream camera/AO/render-scale paths plus addon/report/search hooks" without
noting that upstream's F3-before-map order was dropped.

Fix: move the F3 handler (lines 3662-3672) above the map block, matching upstream.

---

## MEDIUM

### M1 — The bindpage test now asserts `"(null)"` placeholders instead of real labels

`crates/eqoa/src/bindpage.rs:880`, `:915`, `:924`.

Upstream's bindpage test used `hide_ui`, whose label is the literal `"Hide UI"`:
`assert_eq!(page.note.as_deref(), Some("Hide UI: F2. H does nothing now."));`

The merge changed the row under test to `map`, whose label is the **game word**
`{421}` (`crates/eqoa/src/binds.rs:155`:
`row("map", "{421}", &["m"], …)`). In the test fixture the word table is empty, so the
label resolves to `"(null)"`, and the assertions were rewritten to match:

```rust
assert_eq!(page.note.as_deref(), Some("(null): F10. M does nothing now."));
assert_eq!(page.note.as_deref(), Some("(null): |(. (null) has no button now."));
assert_eq!(page.note.as_deref(), Some("(null): |9. (null) has no button now."));
```

Upstream `91f9e72` and `origin/master` both still use `hide_ui` here
(`origin/master:crates/eqoa/src/bindpage.rs`, "Hide UI: F2. H does nothing now."), so
this is a fork-side divergence, not an upstream port.

Failure scenario: the assertion now pins a `(null)` placeholder rather than the row's
name. If a later change breaks the Controls page's row-label lookup (e.g. the
`{421}` word substitution), the test still passes because it expects `(null)`. The
three assertions have lost the ability to detect exactly the class of bug they were
written for.

Fix: keep the row under test as `hide_ui` (as upstream does) so the note text stays
meaningful; if the map row must be covered, add a *separate* assertion with the game's
word table populated.

### M2 — The default-key change from G (group waypoint) to G (attack) has no migration

`crates/eqoa/src/binds.rs:123` (`attack` → `["g"]`), `:172` (`map_group_waypoint` → `["o"]`),
`:136` (`jump` → `["space", "v"]`).

The fork's defaults were `attack` = Space, `map_group_waypoint` = G
(`4ebe0f7:crates/eqoa/src/binds.rs:109,157`). The merge adopts upstream's
(attack = G, group waypoint = O, jump = Space/V).

The `Binds::from_text` migration machinery handles **saved** layouts correctly — I
traced it and it is genuinely good work. `key.attack=q\nkey.talk=space` yields
attack=q, talk=space, jump=g (the freed G cascades into the jump), and
`macro.3.key=g` keeps G while the attack is left keyless
(`crates/eqoa/src/binds.rs:1670-1694`). The `KeyEdge::space_attacks` fallback keeps
Space working as the attack wherever no jump is possible
(`crates/eqoa/src/viewer.rs:5107`).

But the migration only covers rows the user **explicitly saved**. A fork user on the
default layout has no `key.map_group_waypoint` line in `keys.opt`, so their group
waypoint silently moves from G to O, and G silently becomes the attack.

Failure scenario: a fork player who used G on the map to drop a group waypoint — the
fork's default, so nothing was ever saved — upgrades and presses G expecting the
group waypoint. Instead they swing at the target, and the group waypoint must now be
dropped with O. This is the owner's explicit decision ("use O group waypoint",
"G attack") and the Space fallback mitigates the Space half of it, but the G half is
silent. Worth a line in `FORK-UPDATE-NOTES.md` so players are told, and worth
considering a `keys.opt` migration that maps a *default-position* group waypoint on G
to O.

### M3 — `fork_options::change` runs the "Mods & Addons" **actions** on LEFT/RIGHT

`crates/eqoa/src/options.rs:797` → `crates/eqoa/src/fork_ext/options.rs:89` → `:93-106`.

```rust
// options.rs
_ => { fork_options::change(&mut c, self.item); }
```
```rust
// fork_ext/options.rs
pub(super) fn change(c: &mut ClientOptions, item: i32) -> bool {
    match item {
        …
        _ => return activate(item),   // 1051 / 1052 / 1053
    }
    true
}
```

`WndOptions::key` maps LEFT (`17|136`) and RIGHT (`18|137`) to
`self.change(o, ±1, …)`, and the Extended page's `_ =>` arm reaches
`fork_options::change`, which falls through to `activate` for the three action rows
("Open Mods Folder" 1051, "Combat Parser" 1052, "Addon Manager" 1053).

Failure scenario: on Extended → Mods & Addons, highlighting "Open Mods Folder" and
pressing LEFT or RIGHT opens the user's mods folder in the OS file manager; on
"Combat Parser" it launches the parser process; on "Addon Manager" it requests the
addon manager to open. The rows are meant to be activated by ✕ (`select`), not by
left/right.

**Attribution: pre-existing**, not introduced by this merge — the same
`_ => { fork_options::change(&mut c, self.item); }` and the same `activate` fall-through
are in `4ebe0f7:crates/eqoa/src/options.rs`. Reported because the merge touched
exactly these lines (adding the 1054/1055 masters to the same `change` fn) and because
the fix is one line (`activate` should not be reachable from `change`).

### M4 — The `CursorLeft` handler calls `p.map.let_go()` / `mini_hover(None)` twice

`crates/eqoa/src/viewer.rs:5267-5271`:

```rust
if let Some(p) = self.play.as_mut() {
    p.map.let_go();
    p.map.mini_hover(None);
    p.chat_let_go();
    p.map.let_go();        // duplicate
    p.map.mini_hover(None); // duplicate
}
```

Harmless (both calls are idempotent) but it is a merge artifact and it obscures the
fact that `p.chat_let_go()` is the one that matters between them. Fix: delete the
duplicated pair.

---

## LOW

### L1 — `render/src/lib.rs` is byte-identical to upstream; the report's wording is misleading

`docs/plans/sync-2026-11-09-report.md:31` says "`crates/render/src/lib.rs`: Retain
upstream renderer exports and scaled capture callback; circular map renderer remains
fork-owned."

`git diff 91f9e72 3307f50 -- crates/render/src/lib.rs` is **empty**. The merged file is
upstream's verbatim; the circular minimap lives in `crates/render/src/map.rs` and
`crates/render/src/ui.rs`, which do differ (+12/-2 and +87/-4 vs upstream).

I verified nothing was lost: an identifier-set diff of the fork's `render/src/lib.rs`
against the merged one shows **zero** fork-only `fn`/`struct`/`const`/`enum` names
(83 fork identifiers, 113 merged, 0 lost). The fork's version was simply an older
upstream state. The report's "retain upstream renderer exports" is accurate; the
implication that the file was hand-merged is not. Worth a note so a future reader does
not look for fork edits there.

### L2 — Two upstream test lines removed from `net/src/session.rs` (behaviour-preserving)

`crates/net/src/session.rs` — upstream's `91f9e72` quest-negotiation tests contained:

```rust
s.agree_quest_marks(false);
assert_eq!(answers(&mut s), [], "the Quest Helper off from the start: nothing answered, as today's");
```
(and the same for `agree_quest_progress`). Both lines are gone in the merged tree.

This is **correct**, not a weakening: the merge restored upstream's own
`unwrap_or(false) == want` semantics in `agree_quest_marks` / `agree_quest_progress`
(`crates/net/src/session.rs`, and upstream `91f9e72` has the same expression), whereas
the fork's `4ebe0f7` used `self.quest_marks_asked == Some(want)`. Under upstream's
semantics, `agree_quest_marks(false)` with `asked == None` returns early and sends
nothing — so the removed assertion would still hold, but the merged code makes the
"silent before the first request" property explicit. The merged tests cover the
"asked once for each change of mind" and "a new offer is answered afresh" behaviour,
which is the substantive part.

**Worth noting for the record**: the merge changed a fork behaviour here. The fork's
`== Some(want)` would send a stop ("0") when the Quest Helper is turned off before the
first request; upstream's `unwrap_or(false) == want` stays silent. The report's claim
("quest negotiation remains silent if disabled before the first request and stops an
existing request", `docs/plans/sync-2026-11-09-report.md:42`) describes upstream's
semantics, so this is the intended direction — but it is a behaviour change to the fork
and should be in `FORK-UPDATE-NOTES.md`.

### L3 — The GM-travel world index is not checked against the player's world

`crates/eqoa/src/groupmark.rs:280-295` (`take_map_travel`) forwards
`spot.world as u32` to `Session::map_travel_to`. `spot.world` is the map's currently
displayed world (`crates/eqoa/src/map/mod.rs:552`), which may be a world other than the
player's. The server is documented to check the GM's level and move the character
there, so a wrong world is the server's to refuse; the client only rejects a negative
index. This matches upstream `476fdac` exactly, so it is not a port defect — noted only
because it is the one place where a client-side validation could have been added.

### L4 — `tools/sync_upstream.py` now honours an externally-set `CARGO_TARGET_DIR`

`tools/sync_upstream.py:311`:
`CARGO_TARGET_DIR=os.environ.get("CARGO_TARGET_DIR", str(target))`.

Correct and owner-requested, and the new regression in `tools/test_sync_upstream.py`
(`test_configured_cargo_target_is_kept_with_isolated_config_and_one_thread`) checks both
the configured and the unset case. One nit: the scratch `EQOA_CONFIG` still points at
`root/"target"/"sync-config"` even when `CARGO_TARGET_DIR` is redirected elsewhere, so
the config directory lands in the worktree while the build output does not. Harmless
(the worktree `target/` is gitignored and was cleaned up), but the two are now
inconsistent.

---

## Verified-correct (checked, no issue found)

- **Upstream completeness.** All 471 upstream files exist in the merge (809 total).
  `git ls-tree` diff shows zero missing paths.
- **The AO failures cannot be ours.** `crates/render/src/ao.rs`, `ao.wgsl`, `lit.rs`,
  `lit.wgsl`, `suncast.wgsl`, `sunshadow.rs`, `lightshadow.rs` and
  `crates/render/tests/ao.rs` are **byte-identical** to `91f9e72` (empty diff). The
  fork's renderer changes are confined to `render/src/map.rs`, `map.wgsl`, `ui.rs`,
  `ui.wgsl` and `tests/map_circle.rs`. The report's negative-control claim holds.
- **The `.gitattributes` LF rule is real and necessary.**
  `crates/render/src/sunshadow.rs:318` defines
  `const SUN_LINE: &str = "    light += … * i.base.w;\n";` — an LF-terminated literal.
  `shadowed_lit` (`sunshadow.rs:336-341`) does `lit.replace(SUN_LINE, …)` and returns
  `None` when the pattern does not match. On a Windows checkout with
  `core.autocrlf=true`, `lit.wgsl` arrives CRLF and the replace misses, so the receivers
  module is never produced. The rule
  `crates/render/src/*.wgsl text eol=lf` (added in `ee7ffa6`) covers all 19 `.wgsl`
  files (all live directly in `crates/render/src/`), and the committed blobs are
  LF (0 CR bytes). This fixes an upstream bug, and the tools regression
  (`test_shader_checkout_keeps_lf_when_windows_conversion_is_enabled`) exercises it
  against a real `git init` fixture with `core.autocrlf=true`.
- **GM travel is a faithful port of upstream `476fdac`.** `crates/net/src/session.rs`
  (`OP_MAP_TRAVEL` 0x7E34, `OP_MAP_TRAVEL_TO` 0x7E35), `crates/eqoa/src/map/mod.rs`
  (`ctrl`/`travel`/`travel_ask`, `Map::click`, `take_travel_ask`),
  `crates/eqoa/src/groupmark.rs` (`take_map_travel`),
  `crates/eqoa/src/map/chrome.rs` (`LEGEND_TRAVEL`, `Chrome.travel`),
  `crates/eqoa/src/map/mini.rs` and `crates/eqoa/src/viewer.rs` are line-for-line
  equivalent to upstream `476fdac`'s diff.
- **The GM travel request is safe.** The capability is **offered only** — `OP_MAP_TRAVEL`
  sets `self.map_travel = on` and sends nothing back; no auto-answer exists. The ask
  path is `Map::click` → `if self.ctrl && self.travel && !finger` — the Ctrl (or
  ⌘/Super) modifier state is read from the window's `ModifiersChanged` and
  `MouseInput` events (`viewer.rs`, `m.state().control_key() || m.state().super_key()`),
  so it cannot fire without the user physically holding the modifier; the touch path is
  excluded by `!finger`. The ask is one-shot (`travel_ask.take()`). The packet is
  2 + 4 + 4 + 4 = 14 bytes: opcode, `u32` world, `f32` x, `f32` z — `x.to_bits()` and
  `z.to_bits()` are byte-identical to the `f32` LE encoding, and the net test asserts
  the exact bytes. A focus loss clears `map.ctrl` (`viewer.rs:2774-2776`), so a lost
  Ctrl release (Wayland) cannot leave travel armed.
- **The quieter quest rim is a faithful port.** `MarkKind::on_rim()`
  (`crates/eqoa/src/map/mini.rs:155-157`) returns true only for `QuestStep`;
  `place_marks` (`:410-422`) places quests-to-take only where they lie and keeps only
  steps on the rim; the map's own marks sort `?` over `!`
  (`crates/eqoa/src/map/mod.rs`, `marks.sort_by_key(|&(kind, _)| kind != 1)`). Matches
  upstream `476fdac`.
- **The map cache precedence is upstream's.** `EQOA_MAP_CACHE` → `EQOA_CACHE/map` →
  platform cache (`crates/eqoa/src/map/store.rs:53-71`), byte-identical to upstream.
- **Option IDs do not collide.** Upstream's `EXTENDED_ITEMS` (39 rows, 12-57 with gaps
  at 28/31/41-45) plus the fork's `ITEMS` (31, 42, 1043-1055) are disjoint: upstream's
  41-45 rows were removed from the shared table, and 31 and 42 were upstream gaps.
  The masters use 1054/1055. `every_extended_row_has_its_own_item`
  (`crates/eqoa/src/options.rs:2518`) enforces uniqueness over the combined registry.
- **Saved-config keys all survive.** `crates/eqoa/src/settings.rs:1019-1027` writes the
  seven map/quest keys exactly once each through one table-driven loop; the follow-up
  `ee7ffa6` removed the duplicate literal blocks that `3307f50` had introduced (which
  would have written `minimap=off`/`map_markers=off`/… twice). All seven keys parse
  symmetrically (`settings.rs:1173-1186`) and `from_text`/`to_text` round-trips are
  covered by `the_map_and_the_quest_helper_kept_only_when_off` (`settings.rs:2457-2478`),
  which is upstream's test preserved verbatim.
- **No upstream serialization was dropped.** Upstream's `options_with_the_lines_of_rows_gone_still_load`
  was replaced by the fork's `map_markers_kept_only_when_off` + `quest_markers_kept_only_when_off`
  + `quest_tracker_kept_only_when_off` + `minimap_kept_only_when_off` +
  `stats_box_kept_only_when_on` + `mana_bars_kept_only_when_off`
  (`settings.rs:2138-2196` and `:2436-2455`) - a strict superset of upstream's coverage, because the
  fork retains the rows upstream removed.
- **The API hooks are exactly once and in the right places.** In `crates/eqoa/src/play.rs`
  (merged): `fork_hooks::begin_iteration` at 2595, `frame_library` at 2596,
  `begin_game_event` at 2617, `end_game_event` at 3193, `observe_text` at 5148 —
  exactly one each, and `begin_game_event`/`end_game_event` still bracket the native
  event `match`. Identical to `4ebe0f7` modulo line offsets. `prepare_controls` is
  called once per branch in `viewer.rs` (2892, gameplay; 3301, pregame/modal) —
  two distinct functions, not a double call. The whole fork API surface
  (`fork_ext/api_capture.rs`, `api_capture_tests.rs`, `fork_ext/addons/*`) is
  **unchanged** by the merge except `fork_ext/options.rs` (the two master rows), so
  the perf-fix gating cannot have been bypassed.
- **Master-off paths release resources and claim no stale input.** `Map::off()`
  (`crates/eqoa/src/map/mod.rs:383-395`) is a superset of upstream's: it always
  `close()`s (clearing `held`, `press`, `square_hold`, the search and the suggestion
  form), clears the pending travel and group asks, calls `mini_off()`, and drops
  `layers`/`world`/`icon.tex`/`reported`. `Play::map_frame` (`play.rs:8113-8120`) calls
  `self.map.off()` and returns when `!crate::settings::map()`. The follow-up
  `ee7ffa6` added `master_off_releases_world_and_closes_search_without_losing_saved_waypoints`
  (`crates/eqoa/src/map/tests.rs:1031-1049`) covering exactly this, including that the
  saved waypoint survives.
- **The master switches gate their parts correctly.**
  `settings::minimap()` = `map && minimap`, `quest_markers()` =
  `quest_helper && quest_markers`, `quest_tracker()` =
  `quest_helper && quest_tracker` (`crates/eqoa/src/settings.rs`). Turning a master
  off leaves the parts' own saved settings untouched (asserted in
  `the_map_and_the_quest_helper_kept_only_when_off`), and the Quest Helper's own
  tracker still works with the Map off (`tracker_quests` reads only
  `quest_tracker()` and `right_hud_up()`).
- **The quest negotiation is upstream's.** `Play::agree_quests` is byte-identical to
  upstream (`play.rs`, `session.agree_quest_marks(quest_markers() && map() && !forced)`,
  `session.agree_quest_progress(quest_tracker() && !forced)`), and `Play::guide_on` no
  longer tests `quest_helper()` (the master is applied through `quest_markers()` /
  `quest_tracker()` instead) — consistent with upstream.
- **The bind defaults do not collide.** In the merged `ROWS`: attack=`g`,
  jump=`space`/`v`, map_group_waypoint=`o`, stats_box=`b`, map_search=`f2`,
  map_stop=`f4`, talk=`q`, hide_ui=`h`, map=`m`. All distinct; no existing key was
  reused. F2 and F4 were free in both the fork and upstream's default layouts (upstream
  only used them in *test* bindings, which the merge correctly moved to `f10`/`y`).
- **The map's O key matches the legend.** `crates/eqoa/src/map/mod.rs:1839-1841` maps
  `"o" => (KEY_GROUP, Hold::Once)` and the chrome's group legend reads
  "O, Q held or Shift+click Group waypoint" — the fork's `g` was correctly changed to
  `o`, matching upstream `53c5cf5`.
- **The map's "Center" legend follows the jump, not the attack.**
  `crates/eqoa/src/map/chrome.rs` uses `key("jump", "Space")` for the Center label (it
  used `key("attack", "Space")` before the follow-up), and `ee7ffa6` added
  `center_legend_follows_the_space_row_after_attack_moves_to_g`
  (`chrome.rs:455-475`) to cover it. The map's own keyboard table still maps
  `"space" => (key::CROSS, Hold::Once)` (`map/mod.rs:1835`).
- **`main.rs` / `touch.rs` / `tracker.rs` / `mapsuggest.rs` / `groupmark.rs`** are
  clean: `main.rs` gains only `mod statbox; mod fork_ext; mod report_policy;` and the
  two `report::secret` calls (fork features); `touch.rs` only gains the `dpad` field and
  reorders `Default`; `tracker.rs` is +1/-0; `mapsuggest.rs` adds the fork's keyboard
  suggestion form and its test.
- **The footprint budget and hook count were updated honestly.**
  `docs/fork-footprint.json` records the new `options.rs` budget (46/142) and the
  `fork_options::items` hook count 4 → 6, plus a new `texpack.rs` entry for the
  Windows fixture correction.
- **`crates/eqoa/Cargo.toml`** unions correctly: `ab_glyph` (fork theme font),
  `mlua`/`dear-imgui-*` (fork addons) plus `Win32_System_Com`; `crates/net/Cargo.toml`
  gains `sha2 = "0.10"` (needed by the fork's `0x7F53` HMAC client check, which is a
  pre-existing fork feature, not part of this merge).

---

## Notes for the owner

1. **The two AO failures are genuinely upstream's.** The AO code, shaders and tests are
   byte-identical to `91f9e72`, so no fork change can plausibly affect them. The
   report's negative-control method (extract `91f9e72` with `git archive`, run the
   same test with the same target/lock) is the right one and the conclusion holds.
   The `.gitattributes` LF fix in `ee7ffa6` is a **separate, real** upstream bug (the
   `SUN_LINE` LF literal vs a CRLF checkout) that the merge fixed correctly — do not
   confuse it with the AO failures.
2. **`origin/master` has moved to `14636b2`** (four commits past the pinned
   `91f9e72`). This review covers the pinned merge only, as the report states.
   `e8d7c7d` ("Touch controls adjustable") and `14636b2` ("Light Shadows idle") are
   **not** in this merge and would need a separate sync.
3. **H1 should be fixed before merging into `develop`.** H2/H3/H4/M1/M4 are cleanup and
   should ideally land in the same follow-up commit so the branch does not ship a
   dead code block.
