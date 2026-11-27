# Backlog

## Make automatic updates visible to players

- **Status:** Deferred; keep Forge compatibility as the current priority.
- **Reported:** 2026-10-28, during the 0.1.0 to 0.1.1 update.
- **Observed:** The user saw no update message, but confirmed that a new executable downloaded and the previous executable was renamed to an old copy. This confirms update file activity; the running version was not independently checked.
- **Follow-up:** Review notifications for startup updates and updates downloaded during play. Make it clear when an update is downloading, ready for restart, successfully installed, or failed. Show the installed version so players can confirm the result without inspecting executable files.
- **Acceptance:** Verify both update paths from an official release with a scratch profile. A completed startup update gives a visible version confirmation; an update downloaded during play provides a clear restart notice. Failure or rollback does not report success. Preserve existing profile data and executable recovery behavior.

This entry records future work only; no updater behavior changes are included.

## Route guidance on the minimap

- **Status:** Deferred; full Wereoxxs compatibility remains the first public dev-build gate.
- **Requested:** 2026-10-30.
- **Goal:** Show a line on the minimap following a usable path to the selected destination, rather than only a straight bearing. The player follows the route manually. A next-turn arrow or world-map route can be optional presentations of the same route data.
- **Initial scope:** Investigate recorded/authored routes as a manageable first version, with automatic obstacle-aware route calculation as a later expansion. Reuse existing destination selection and waypoint tracking where suitable.
- **Technical investigation:** The map uses cached overhead terrain images; actual navigation needs the client's underlying 3D collision/walkability data or a verified route graph. Assess slopes, steps, character clearance, bridges, caves, water, entrances and streamed-zone connections before promising arbitrary destinations.
- **Acceptance:** Demonstrate routes that can actually be walked, including representative turns around obstacles. Keep the path aligned with the minimap and the chosen destination. Make missing or unsupported routes clear; do not present a straight line through obstacles as a calculated walkable route.
- **Autorun:** Lower priority and optional future work. It is not required for the initial route-guidance feature; steering, obstruction recovery and movement automation are outside that first scope.

This entry records future work only; no navigation or movement behavior changes are included.

## Indoor map: dungeons, caves and buildings on the minimap and M map (owner, 2026-11-03)

- **Status:** Backlog; the plan package is ready: `docs/plans/INDOOR-MAP-PLAN.md` (scout findings, a draft 5-line description, design, steps, risks).
- **Problem:** indoors the minimap shows grey "not drawn" or a flattened roof. The map is built from outdoor terrain tiles and never reads room geometry. Forge's `mini_map` drew interiors from the live room geometry with a ceiling cut.
- **Plan:** phase 1 the minimap, phase 2 the M map. The owner approves the 5-line description before the build.

## Second zone in a 2,000-unit cell missing from the map (bug, found 2026-11-03)

- `map/cells.rs` keeps one zone per cell: "A second zone in a cell is left out of the map". Any zone whose middle shares a cell with another is never drawn. Fix separately from the indoor map; first list which zones are affected (all six worlds).

## Save problem reports to the Desktop

- **Status:** Deferred until the Wereoxxs dev-build gate is met.
- **Requested:** 2026-10-30.
- **Goal:** Players who are not technical can hand the owner everything needed to diagnose a problem, with no server. F3 (or the HUD's Report a bug) and the next-start crash offer keep upstream's dialog (screenshot, "what happened" note, log tail), but **Send becomes "Save to Desktop"**: one `EQOA-Enhanced-report-<date>.zip` with the note, screenshot, `eqoa.log`, `eqoa.prev.log`, `mods/addons_error.log`, any waiting crash files, then open the Desktop with the file selected. The player shares that one file however they like.
- **Clearly Enhanced, never upstream:** the file name, the report's first lines, and the first line of every log say "EQOA Enhanced <version> (<build commit>), a fork of DabDavis's vanilla client: report to EQOA Enhanced (LoopRook), not to the vanilla client", plus the list of enabled mods/addons and enhanced features. Upload stays disabled (report policy), so a report can never reach upstream's reportd. The owner triages first; a bug goes upstream only if it reproduces in the vanilla client, and only by the owner's decision, labelled as coming from Enhanced.
- **Privacy:** keep upstream's scrubbing (`report::secret`: account name, password, the home directory's path). Nothing leaves the computer unless the player shares the file.
- **Implementation notes:** reuse `report.rs`'s gathering and dialog through a fork hook (fork_ext), replacing only the transport; no new server, no telemetry.
- **Later option (owner, 2026-10-30): send to the owner's Discord.** A Discord webhook can post the same zip into a private channel, with no server of our own. Trade-offs to settle first: the webhook URL ships inside the client, so anyone can extract it and spam the channel (rotate it, rate-limit, and keep Save to Desktop as the default); Discord's upload size limit (trim logs); players' logs then sit on Discord (consent text on the button). Build it on top of the same report file, opt-in per report.
- **Acceptance:** F3 → note → Save produces the zip on the Desktop with all parts and the Enhanced header; a forced crash's next start offers the same; tests prove secrets are absent and that no network request is made.

- **Owner test 2026-11-01 (develop 7a531b8): PASSED.** The zip saved and the folder opened. Lead check of the owner's zip: no home path, and 0 hits for the saved account and password in any text file. Follow-ups: (1) the file name uses UTC (20261101-045745 for local 00:57); use local time. (2) The manifest lists `resources` as an enabled mod (it is the shared resources folder, not an addon).

## Minimap search button disturbs autorun / follow (bug)

- **Status:** Reported 2026-10-31; read-only diagnosis in progress. Separate from the Wereoxxs gate.
- **Observed (owner, live play):** pressing the minimap's search button while auto-running, or at least while following someone, changes the character's movement ("makes your character start"; whether it starts moving or stops is being confirmed).
- **Acceptance:** clicking search (and opening/closing the search box) leaves autorun and follow exactly as they were; a test reproduces the old behaviour and fails before the fix.

## NPC search finds nothing when map-entities.json is not beside eqoa.exe (bug, released 0.1.1)

- **Status:** Reported 2026-10-31 by a player (first report). Awaiting the owner's decision on a 0.1.2 patch.
- **Cause (verified in code):** `map/search.rs` `ensure_loaded` reads `EQOA_MAP_DATABASE` or `map-entities.json` beside the executable; when neither exists it loads nothing silently (no message, no log line), so NPC searches return nothing (XP camps and coaches still appear). The 0.1.1 release offers both the ZIP (with the catalog) and the bare `eqoa-enhanced-windows-x86_64.exe` (the updater payload, no catalog); running the bare exe, or running eqoa.exe from inside the unextracted ZIP, produces this.
- **Proposed fix:** embed the pinned FrontiersForge catalog (MIT, notice already shipped; ~1.7 MB, pinned SHA256 per tools/build_player_release.py) in the executable as the fallback, keep the file beside the exe as an override; when no catalog is available, say so in the search window and log one line; label the bare exe on the release page as the updater's file.
- **Acceptance:** search returns NPCs when run from the bare exe and from inside an unextracted ZIP; a test removes the side file and still finds a known NPC; the missing-catalog case shows a visible message.

## Chat tabs that pick the channel you send to (our own mod)

- **Status:** Idea for the fork's own UI mod; not a Wereoxxs compatibility gap.
- **Requested:** 2026-10-31.
- **Finding:** in Wereoxxs (reference `modules/chat_window.lua` ~238-260, `wereoxxs_ui.lua` ~855-860) the All/Party/Guild/Say/Shout tabs send text unchanged (no channel prefix); only tell tabs prefix `/tell <name>`. Our native port matches that.
- **Goal:** in our own chat mod, the selected tab sets the outgoing channel (Party -> group, Guild -> guild, Shout -> shout, Say -> say); decide what "All" does (last channel used, or say) and show the active channel next to the input.

## Mod profiles per character or role, and separate settings per client (multibox)

- **Status:** Requested 2026-10-31; design pending (a scout will first check how Forge/UiForge and Wereoxxs handle profiles on PCSX2).
- **Problem:** all clients on one PC share one settings/profile file per mod and one window-layout file (imgui.ini), so two clients open for two accounts overwrite each other's settings and layouts.
- **Goal (owner):** save named profiles per character or role and choose which to load, as players did on PCSX2; each client keeps its own.
- **Owner decisions (2026-10-31):** load automatically per character; profiles include window positions. Named profiles (e.g. Healer, Tank) can be shared by many characters (a druid and a shaman both on "Healer"); each character remembers its profile; a character without one gets the default plus a quick pick/save choice; optional "use for all characters of this class". Each client loads its own character's profile, which ends the multibox clash.
- **How PCSX2 did it (scout, 2026-10-31, reference file:line in the agent log):** UiForge saves named profiles as `<scripts>/profiles/<name>.profile.lua` (serpent Lua, atomic), each holding every enabled script's state plus the ImGui window layout (SaveIniSettingsToMemory); the player picks via Settings > Select/Save Profile; "preferred" auto-load is per executable name, not per character; two PCSX2 instances only avoid clashing via separate install folders. Wereoxxs's Save/Load table holds windows, bars, frames, compass, meter and chat configs, with no per-character logic.
- **Our design:** named profiles in `<config>/addons/profiles/<name>.json` holding {enabled addons, each addon's Save state, the window layout ini}; a character map `profiles/characters.json` keyed by server + character name (never the account) -> profile name, optional class defaults; at world entry load the character's profile (else Default, with a quick pick/save prompt in the Addon Manager); Save writes the active profile; the global addons/imgui.ini and per-addon single profile become the "Default" profile (migrate the existing files once); each client holds its own active profile, so multibox no longer clashes (two clients on the SAME profile: last save wins, so show which profile is active). Later: a converter importing PCSX2 `.profile.lua` files.
- **Also decided:** the eqoa/1 compatibility promise with a capability query is adopted; hook functions that do nothing must fail honestly (false plus a diagnostic) instead of reporting success.

## Release policy for Forge work (owner, 2026-10-31)

- Bug fixes in released features go to stable as patch releases (hotfix branch from enhanced, merged back to develop).
- New Forge compatibility work stays on develop and ships when parity is reached for the next set of mods.

## Creature journal for map search (proposal, awaiting the owner)

- **Status:** Proposed 2026-10-31; automatic sharing approved in principle by the owner; build not started.
- **Background (Wereoxx, relayed by the owner):** `map-entities.json`, the catalog our map search uses, is a one-time scrape of the Sandstorm server's entity list from when it was less locked down. It is very incomplete, and there is no other source unless the server devs expose the entity database again. Wereoxx's world_map mod fills the gaps with a journal: every player records the creatures they see and can share the file with others.
- **Reference:** `world_map/modules/journal.lua` (pinned FrontiersForge reference, MIT). File `journal.json`: `{ "<name>": { "<entity id>": { "world": w, "level": l, "pos": [x, y, z] } } }`. Nameless creatures are recorded under `(unnamed)`. Recording happens in memory, with a flush after a quiet period. A known id is never rewritten. A new id is skipped when the same name at the same level was already recorded nearby. Merging a friend's file adds only missing names and ids; your own data wins. Disposition is not stored in this format.
- **Proposal:** native recording in our client (fork-owned `map/` code, a hook only where entities are seen). Map search shows catalog results plus journal results. Import/export uses the same `journal.json` format, so files can be swapped with Forge players on PCSX2.
- **Privacy:** the journal holds only what the player saw. No account, character or path is stored. Sharing is manual (export a file); nothing is sent anywhere.
- **Owner decisions (2026-10-31):** sharing is automatic and on by default, with no setting. The service runs on the owner's Railway account, which is already set up.
- **Shared community journal (plan):** 1) native journal (offline; file-compatible with world_map.lua). 2) the client downloads a community journal from our service, the way it checks for updates, and merges it in. 3) the client uploads only new sightings (name, zone, level, position, entity id) in small batches with a random install id; no account, character name or paths. Service-side: schema and range checks (levels 0-255, the game's level byte, as the client accepts; positions inside the zone), size caps, per-install and per-IP rate limits. An entry enters the public list only when at least 2 different installs report it, which guards against poisoning. The service is labelled EQOA Enhanced and never involves upstream. The client carries no write secret, because anything shipped in the exe can be extracted.
- **Decided (2026-10-31):** no disposition or con colour in shared data. Both depend on the viewer (faction/alignment, and the viewer's level), so shared values would mislead. Con colour is computed locally from the recorded level. Telling Wereoxx is optional and not planned. Release notes should mention the automatic sharing (recommended; the owner has not objected).
- **Acceptance (draft):** seeing a creature records it once; walking past the same spawn again adds nothing; a merge adds only new entries; search finds a journal-only creature; a round-trip with a file written by world_map.lua loads unchanged.

## Match our UI to the EQOA Parser style (owner, 2026-11-01)

- **Request:** the client's own UI (map search, Addon Manager, profiles and so on) was meant to look like the owner's EQOA Parser (`<repo:eqoa-parser>`), which took its rules and inspiration from Track Level Companion (`<repo:track-level-companion>`). Both are on the owner's GitHub.
- **Status:** deferred, not urgent. Next step: a read-only scout extracts the style rules (palette, fonts, spacing, components) from both projects into a short style guide, then one restyle pass over the fork-owned UI.

## XP to the next Class Mastery point (owner, 2026-11-01)

- **Request:** at level 60, show how much XP is left until the next Class Mastery point, in the HUD/XP readout and for mods (e.g. the Modern UI XP bar, which today shows a full bar at 60).
- **Status:** not started. Next step: find where the client keeps Class Mastery XP and the per-point cost (decomp/live packets; Forge exposes CM values, see Astra's note on hard-coded CM values), then show 'CM: x / y' in the XP readout and expose it under the eqoa namespace for mods.

## Weapon-set bar (and bars 3/4) inside Wereoxxs UI (owner, 2026-11-01)

- **Request:** Sandstorm added a 4th hotbar for weapon sets. Wereoxxs UI draws only Forge's three hotbars (bars 1-2 and items), so with its ability bars on, our weapon-set page (HUD menu 3, and our extra bars 3/4, menus 4-5) is hidden with nothing in its place. The owner wants them drawn inside Wereoxxs's own bar style. Not unhidden as the game's bars.
- **Plan:** expose the weapon sets (and extra bars) to Lua under the `eqoa` namespace (slots, icons, current set, use), then patch the bundled Wereoxxs ability_bars.lua (MIT; note it in bundled-mods/SOURCE.txt) to draw a 4th bar, and offer the patch to Wereoxx. Medium: one agent task plus a review.
- **Status:** shipped in 0.4.1 (Wereoxxs weapon-set bar).

## Wereoxxs weapon-set bar: one selection ring, settings with the other bars (player report, 2026-11-04)

- **Report (a player, via the owner, with a screenshot):**
  1. The weapon-set bar has its own selection ring. When you move onto it, Wereoxxs's ring stays stuck on the bar/ability you had selected before, so two rings show at once.
  2. The weapon-set bar's ring ignores the custom colours set for the selection indicator.
  3. Its options sit apart from the 3 bars in Wereoxxs settings. It would be more intuitive under "Ability bars", with a "Show bar 4" option there.
- **Likely cause (unverified):** our patch in bundled-mods/wereoxxs_ui (weapon-set bar, 0.4.1) draws its own highlight from the `eqoa` weapon-set selection. Wereoxxs's ring follows the HUD carousel's selected bar/slot, which doesn't move to the weapon-set page.
- **Fix idea:** treat the weapon-set bar as bar 4 in ability_bars.lua. Then there's one selection model: when the carousel is on the weapon-set page, the ring moves there and uses the configured colour, and the weapon-set bar's own highlight is dropped. Move its settings into the "Ability bars" section as "Show bar 4". Update the bundled file's shipped hash so unedited installs get it.
- **Status:** owner decision: not now; aim for the next round of Wereoxxs/mod-bar work (with the Modern/ARPG weapon-set bars and click-to-cast).

## Live 'mob is up' alerts shared between players (owner idea, 2026-11-01)

- **Idea:** a client can only see a creature when it is within the server's entity range, so the map can't tell whether a far-off mob is up. Instead, players' clients report sightings of creatures as they see them, and anyone searching for that creature gets a pop-up ('<Mob> seen up near <place>, 2 min ago').
- **Builds on:** the creature-journal sync service (steps 2-3, Railway). A sighting is a tiny message (name, zone, position, time).
- **Shape:** clients send 'seen alive' (and 'seen dead/despawned', if visible) for creatures as they appear; the service keeps the last sighting per creature; a client polls every 30-60 s only for the names it is watching (a watch list from map search). Polling, not live sockets: simpler and cheap.
- **Questions:** staleness wording; spoofing (require 2 reporters, or mark single reports as unconfirmed); privacy (a sighting implies some player is near there: send no player info, maybe round the position); opt-out; cost on the Railway hobby plan (likely small at our scale, but measure).
- **Range:** not adjustable by us. The server decides which creatures it sends (log: 'NPC/PC Display 200, the server's most 64'), so the client already gets everything the server allows.
- **Status:** idea; after journal steps 2-3. Medium-large.

## Click to cast from mod ability bars (owner, 2026-11-01)

- **Request:** in Wereoxxs UI, clicking a weapon-set slot switches set (0.4.0), but clicking an ability slot does nothing; the same goes for the other UI mods. The owner wants click-to-cast on the mods' ability bars.
- **Plan:** check what Forge exposes for using an ability from Lua (the reference ability/ability_bar modules); if Forge has it, implement it natively through the same path as the game's own hotbar use; if not, add it under the `eqoa` namespace and patch the bundled mods' slot click handlers (as for weapon sets). Together with the Modern UI / ARPG UI weapon-set bars (work in progress, uncommitted in the release-0-4-0 worktree) as the next small patch.

- Local reports in a second window currently collect window 1's log; select that window's own log in a future fix (pre-existing; deferred by the 2026-11-03 sync review).

## A small "what the game was doing" section in reports (owner, 2026-11-03)

- Quiet bugs can leave no log clue: mode, target, nearby entities and addon HUD claims help explain them.
- Implemented on sync/2026-11-03 as a small fork-owned snapshot, through the existing frame adapter, at most once a second. It includes enabled addons, world/zone, position/heading, map/search and autorun/follow. Account/password/home redaction applies; character names stay.
- `client_state.txt` accompanies local reports when available; crashes carry the last noted snapshot. Upstream's deleted describers remain deleted. **Done** (develop, 2026-11-04); its I/O runs on a background worker (ca93430).

## Crash offer after a forced close: owner saw no offer (2026-11-04)

- The owner force-closed the combined-da9b159 test build (Task Manager Details, End task) and saw no crash offer. That folder's profile/ held only the killed run's eqoa.log (no `[eqoa] closed`, no eqoa.prev.log), so the game had not yet been started again from the same folder; the offer only appears on that next start. Retest: force-close, start the same folder again; if still nothing, debug `fork_ext/report_core.rs` `ended_without_closing` (reads eqoa.log before logfile renames it).

## Target of Target overlaps a big raid list (2026-11-07)

- **Status:** Deferred by the owner (no UI work now). Ours: the raid list is upstream's,
  the Target of Target bar (hud.rs `TOT_AT` [20, 300]) is Enhanced's.
- **When:** a server with raid groups, Raid List on, Target of Target on, and a raid list
  of 13+ rows (no pet) or 10+ rows with a pet. The list (and the pet bar under it) reaches
  y 314, and the ToT bar is drawn over the last raid row or the pet bar; its click area
  shadows theirs.
- **Smallest fix:** hide the ToT bar while it would overlap (one guard in hud.rs); moving it
  needs layout work. Leave upstream's raidlist.rs untouched.

## Tab opens chat (owner, 2026-11-09)

- **Report:** pressing Tab seems to open chat. Not yet known whether the cause is ours (the client's key
  handling, or the new `eqoa` input claims from round 2) or Wereoxxs UI's own key handling.
- **To find out:** reproduce with no mods enabled (in the upstream client Tab may already be the
  chat/target key; check the key binds), then with only Wereoxxs, then the round 2 dev build versus
  0.4.14. If it only happens with Wereoxxs, look at its Tab handling (input claims and text capture); if
  it also happens with no mods, it is client behaviour and a binds question.
- **Status:** deferred; not investigated. Note it in the round 2 live test if it shows up there.

### Tab opens chat: reproduction (owner, 2026-11-09)

- Tab normally targets. After pressing Enter to open chat and Enter again to close it without typing,
  one Tab press targets and then chat opens again. Casting a spell or opening a menu seems to clear it,
  until the Enter, Enter sequence is repeated.
- Reads like leftover chat or text-input state after an empty close (a stale "chat is open" or key
  capture flag that the next Tab triggers). Not yet known whether it is in the client or a mod (check
  with no mods first, as above).

## Nightly update says it is the same as Stable (owner, 2026-11-09)

- **Report:** when Nightly updates, the message says it is the same as Stable, although the Nightly build
  has newer changes.
- **Likely cause (unchecked):** Nightly and Stable share `VERSION` (0.4.14 on develop, until the next
  release), and the update message and "what's new" come from `FORK-UPDATE-NOTES.md`, whose newest
  section is the last release. The updater (`update.rs`, fork `release.rs`, `EQOA_UPDATE_CHANNEL=
  enhanced-dev`) can tell builds apart by commit, but the text shown to the player does not.
- **To check:** which screen says it (update prompt, what's-new box, version label) and the exact
  wording; whether the Nightly build label shows the commit; then show the build's commit and its own
  unreleased notes section instead of repeating Stable's.
- **Status:** deferred; not investigated.

- **Tab/chat, scout result (2026-11-09):** likely Wereoxxs plus ImGui Tab navigation with one-frame-late
  key capture (`runtime.rs`); predates round 2. Details and the smallest fix: `docs/plans/eqoa-api/tab-chat-scout.md`.
  Queued as a fix on a branch after round 2's slowdown fix merges (same input code).


## Server-added ability bars in bundled Forge UIs (owner, 2026-11-09)

- **Status:** Deferred feature request, separate from the fixed round-2 icon-stacking regression.
- **Current boundary:** Native `hudmenu::HudMenus` has six menu records: ability menus 0/1/4/5, items 2, weapon sets 3. The carousel order is 0,1,4,5,2,3. Expansion bit 1 and the Extended Ability Bars setting permit up to four ability bars with five regular slots each; the first menu can additionally hold four special slots (252-255), for nine entries.
- **Inherited Forge limit:** Pinned FrontiersForge and the native adapter at both 413d1ee and current develop expose `num_bars=3`, zero-based indices 0/1 for abilities and 2 for items. Legacy snapshots cap at five/five/four slots. Menus 4/5 are omitted; the existing selected-bar clamp can highlight the item bar while an extra native ability bar is selected. This is not new in round 2.
- **Bundled UI assumptions:** Avoids Modern UI loops `0..AbilityBar.num_bars-1`, treating bar 2 as items. Wereoxxs prepares three stock bar configs, loops bars 0..3 and reserves 3 for its separate weapon-set extension. Inflating `num_bars` alone would collide with those meanings.
- **Requested work:** Add explicit extended bar/slot metadata and selection mapping, preserve legacy Forge stock indices, and update the bundled UIs to enumerate supported bars with independent layouts. Native `eqoa.hotbar.list()` already exposes the configured carousel with one-based indices and full native slot counts; document and verify that extended contract alongside `eqoa.abilities`, rather than inventing aliases or silently changing Forge's three-bar contract. Item and weapon-set availability must remain explicit.
- **Acceptance:** Exercise one/two/four ability bars, special first-menu slots, items and weapon sets; distinct records/positions and correct active-bar selection after carousel changes; retain saved layouts and stock Forge compatibility. Live-test on a server that persists the extra spellbook slots. No bundled Lua rewrite is included in the live-regression fix.

## Two CI jobs per push repeat each other (owner, 2026-11-09; for the Opus lead)

- Every code push to `develop` starts two workflows: "Enhanced public source checks"
  (`public-source.yml`: footprint, tools tests, workspace tests, an unstamped build, no publishing) and
  "Development build" (`enhanced-dev.yml`: the same gates, then the stamped build, package and Nightly
  publish). Both compile and test the workspace, so a code push costs two runners and about 15 minutes.
- **Question:** keep `public-source.yml` as the gate for pull requests, `enhanced` and tags, and let
  `develop` pushes rely on the Nightly's own gate? Check what else depends on the public check (branch
  rules, release gating in `official-release.yml`) before cutting anything.
- **Status:** deferred; release pipeline, so the Opus lead decides.

## Final task before merging develop into stable (owner, 2026-11-09)

Do this last, before the release that carries round 2 and the sync into `enhanced`:

- **The Stable program must be named "EQOA Enhanced.exe".** The owner's 0.4.14 download is not (the ZIP's
  program carries the old updater-style name). `tools/build_player_release.py` already maps
  `EXE_NAMES["enhanced"] = "EQOA Enhanced.exe" (37dce3a, develop only so far). Verify on the release
  candidate: the ZIP lists `EQOA Enhanced.exe`; a player on 0.4.14 who updates gets a working program
  (the updater replaces whichever file is running: check what happens to the old name, the shortcut,
  the folder's other files); Nightly stays "EQOA Enhanced - Nightly.exe"; the update asset
  (`eqoa-enhanced-windows-x86_64.exe`) still works. Owner checks the downloaded Stable.
- Also in that release: move `## Unreleased` in FORK-UPDATE-NOTES.md to the version's dated entry
  (docs/RELEASING.md), and re-run the live test of Wereoxxs, Avoids Modern UI and the hearts example on
  the release candidate.

## Native capture lacks Stable's player-position fallback (Sol observation, 2026-11-09; untested)

- Found while checking the ability-icon colours (those were only out of range; no regression). In the
  eqoa native capture path (`api_capture.rs` / `native_projection.rs`), the player's position has no
  equivalent of Stable's `self_at` fallback when no walker exists (for example before the walker is
  ready, or in the offline fixtures). Possible effects: position reads as zero for a frame or two at world
  entry, so range-based UI states (out-of-range tints) could flicker or read wrong briefly.
- **To check:** at world entry and after a zone change, watch an ability bar with a target in range;
  and add a test with no walker and a known self position. Low priority until seen in play.

## Upstream AO tests fail on GPUs and on the CI runner (2026-11-09)

- `crates/render/tests/ao.rs`: `the_foot_of_a_wall_darkened_and_open_ground_not` fails on untouched upstream
  91f9e72 on the owner's RTX 3070 Ti (Vulkan and dx12) and on GitHub's Windows runner;
  `the_same_at_freeport_as_at_the_origin` fails on the RTX but passes on the runner. All three
  workflows now `--skip` the first (owner agreed, 2026-11-09). A worker should find out whether the
  assertions assume a particular GPU (plain grey 127 versus 128, a factor difference of 54 against
  a maximum of 6) and, if so, report it to upstream; then drop the skip.

## Round 7 API: conflicts between mod and native key bindings (round 3 review NEW-M2)

- Deferred by the lead on 2026-11-10. Round 3 retains independent mod bindings.
- Define conflict handling across native controls and all enabled mods, including default keys,
  rebound keys, and changes saved from either Controls section. Match native freed/swapped feedback
  and ensure a single key press cannot unexpectedly both Jump and toggle the Stats Box.
- Specify precedence and persistence before implementation; test both directions, multiple mods,
  restart, disabling/re-enabling a mod, and chat/menu suppression. Do not silently steal native keys.

## Local report loses its screenshot after a native graphics-device recovery (2026-11-11)

- Sync 2026-11-11b review finding 3. `fork_report::surface` (adds `COPY_SRC` to the surface and calls
  `report::set_gpu`) runs only from `resumed` (`viewer.rs`, about line 4933); upstream's `remake()`
  (about 3446-3465) builds the new surface configuration without it. After a recovery on native, a
  saved F3/crash report has no picture and still shows the pre-loss GPU name (it degrades quietly, no
  crash). Fix needs one more hook line in `remake`, a shared-file budget check, and a test that a
  recovered surface still captures; low priority until seen.

## Skinnable map (owner, 2026-11-11)

Upstream's map reads its colours and font from one table a skin mod can set (about 30 constants in 6 upstream map files). A neutral seam to offer upstream in round 8; no skin mod until then.

## Zoom the round minimap in further than upstream's limit (2026-11-13)

Owner: zoom felt very limited. The limits are upstream's (`map/mini.rs` `SCALE_LIMITS` 2 to 25 world
units a frame pixel outdoors, `map/indoor.rs` 0.5 to 6 inside), and the mod forwards every wheel
notch whole (`round_minimap_wheel_reaches_both_ends_of_upstreams_zoom_in_whole_steps`). Going
closer than upstream would need a neutral zoom-limit seam (a held view asking for a lower floor) or a
mod-owned zoom the capture honours. Not built; propose it before building.

## Wereoxxs not maintained for now (owner, 2026-11-15)

Wereoxxs: not maintained for now (owner 2026-11-15). Separately, the owner noticed our map in 0.5.3 (live)
zooms in a bit further than round 4's, which keeps upstream's zoom limits; see the zoom item above. Later.

## One home for the fork's options (owner, 2026-11-15)

- **Status:** Backlog; fold into round 8's option-rows seam.
- **Now:** since sync 2026-11-15 upstream's Options > Extended is five groups (Graphics, UI, Other, Touch Controls, Multi-boxing). The fork's Enhanced page (HUD toggles, Mods & Addons) sat in the UI group; since sync 2026-11-26 upstream's UI menu is full (15 categories) and it is in the Other group (Options > Extended > Other > Enhanced), because a top-level entry would mean rewriting upstream's fixed `GROUPS` table.
- **Owner's preference:** everything from the fork in one place.
- **Plan:** the round 8 seam "option rows declared in one table; ids 1000+ reserved for other builds" also lets another build add its own group. Once upstream has it, Enhanced becomes its own top-level entry with no upstream line changed.

## After the PR: batch actions and continuous controls (owner, 2026-11-16)

- **Status:** Deferred until after the round 8 PR (round 6 decision; see the plan's round 6 section).
- **Wanted:** (batches moved into round 6 Part C, owner 2026-11-16) mod-held movement,
  look and controller buttons (`move`, `look`, `buttons`); then autonomous automation (may stay
  Enhanced-only).
- **Note:** round 6 gives one game action per player gesture; these build on its action broker.

## Row/dialog mod actions for bank, merchant, spellbook, quest log and mastery (round 6, 2026-11-16)

- **Status:** Deferred. Round 6 refuses `select_row`/`activate_row`/`close_window`/`menu_command` on these windows with
  `unsupported`, because their pages and sub-dialogs are not readable from fork code (review D, Gemini Hydrogen).
- **To enable:** five read-only accessors proposed in `docs/plans/eqoa-api/round-6b-seams.md` (feat/eqoa-api-r6).
  The typed actions (sell, buy, bank move/money, mastery buy/refund, drop_quest) already cover these windows.

## Helper mod ideas on round 6.5's window takeover (owner, 2026-11-17)

- **Status:** Ideas, not scheduled. Each is a mod, not an API change, unless noted.
- **Helpers** (a mod opens its own hidden session, does the job, closes; a player-opened window stays normal):
  auto-sell junk near a merchant (e.g. from an info bar); auto-loot your kills (Bag Viewer option, round 6.5);
  restock (keep N of an item, buy up to N from a merchant that sells it); auto-bank (deposit crafting materials or
  marked items near a banker); loot filters (always/never take, bag-nearly-full warning); one-key versions of each.
- **Need the takeover extended first** (API work, one window at a time): quest turn-ins (NPC dialogs), trainers,
  crafting.
- **Server limit:** the server decides the range for every request; the owner's round 6.5 test shows whether
  "from anywhere" works.

## Auto-loot beyond "you hit it" (owner, 2026-11-17)

- **Status:** Idea. Round 6.5's auto-loot takes only corpses of creatures you hit, out of combat; others need one click on
  Loot. Later: an Auto-loot setting "my kills only" / "anything the server lets me loot" (ask for every nearby corpse,
  hide the server's refusals; check that chat stays quiet and the server tolerates it) and group kills (only if the client
  can see what group members hit: unverified).

## Bags full while looting: offer to make room (owner, 2026-11-22)

- **Status:** Idea. When auto-loot leaves items on a corpse because the bags are full, offer to make room, as some WoW
  bag addons do (the owner remembers OneForAll-style UIs prompting to delete things): e.g. a small prompt listing the
  cheapest junk items with "Destroy" so the loot fits. Owner answers 2026-11-22: the game has no drop, only destroy;
  keep the game's "are you sure?" (a mod may draw it, never skip it). Needs a destroy action in `eqoa/1` (not there
  today); owner 2026-11-22: destroy is supported like any irreversible step, always through the confirm
  (ui-coverage-audit.md correction). Earliest slot: with or after A1 (the "mod draws the
  confirm" contract), i.e. after the round 8 upstream PR. Builds on the 2026-11-22 auto-loot rule: items left because
  the bags were full are taken as soon as space frees up while the player is still in reach.

## Scripted input: `EQOA_MOD_KEYS` alone still counts as the player's own presses (review 2026-11-18)

- **Status:** Small, not scheduled; test tooling only. `EQOA_MOD_KEYS` alone leaves `input::from_device()` true (`input.rs`
  lists the upstream script variables), so S2's press record treats replayed mod keys as device presses. The record's two
  call sites are upstream lines in `play.rs`; a fix there rewrote both (tried and reverted 2026-11-19). Fix only with a
  route that changes no upstream line (for example the scene tool also setting one of upstream's own script variables),
  or leave it.

## Window takeover and upstream's keyboard-owning windows (sync review 2026-11-18)

- **Resolved for the three server windows, 2026-11-24 (round A11, `feat/eqoa-api-a11`):** a mercenary window, the dye picker and a fishing session the server opens while a hidden session holds the game now act as the game does with a window up (merc: not opened; dye: waits until the session ends; fishing: cancelled), through one hook line each (`fork_hooks::server_window_blocked`). Still open from the note below: `report_viewer.rs:61`'s "not plain play" rule, and NPC dialogs and invitations, which still open over a hidden session on purpose (they carry the server's words, such as a bank's refusal).
- **Status:** For round 6.5 / window replacement. Upstream's mercenary windows and dye picker only open from modes 0, 7, 9
  (`mercplay.rs:142`, `dyeplay.rs:255`); a hidden takeover session leaves mode 0, so they can open while a mod holds a hidden
  bank/loot session (the mod's bank action then stalls until the picker closes; a loot request made in modes 61/63 is
  refused by upstream with 0x0016 and the pending open must be cleared). `report_viewer.rs:61` keeps keys from addons only for
  `merc_win_owns() || dyepick_owns()`: a "not plain play" rule (mode not 0/38) would survive future upstream windows.

## Old Forge chat fallback (2026-11-19)

The fallback drawing path in Wereoxxs `chat_window.lua` still needs a check on an older Forge host.

## Wereoxxs custom font recovery (2026-11-19)

A font restored after going missing is retried after a face or size change or after the addon is re-enabled.

## Mod keys against game keys: accepted leftovers (round 7 Part E, final review 2026-11-20)

- **Status:** Low, not scheduled. None can make a normal press fire both a game control and a mod key.
- The update banner takes Enter, the arrows, PgUp/PgDn, Home and End while it is shown in the world, ignoring
  modifiers: a mod on one of those keys (after the player moved the game control off it) fires as well.
- Chat copy/paste matches the typed letter while the never-a-mod-key list matches the key's place: on a layout such as
  Dvorak a mod on the physical key that types `c` can fire together with Ctrl+C in chat.
- The Macros page (upstream's) accepts a trigger on a mod's key with no note; the macro wins and Controls marks the mod.
- Capturing on a mod row shows "Press the key for ." without the mod's name (upstream draws that line).
- With Auto Save Options off, leaving Options by a route that neither saves nor cancels and then restarting leaves a
  mod's new key saved beside the game's old key; the who-wins order settles it and the row is marked. Closing it fully
  needs two more upstream lines (`settings::save`, `keep`), refused 2026-11-20.
- Marks for keys read from saved settings use a US-layout table until that key has been pressed once this session.
- One enabled mod with no key and no key listener still records the key in the runtime's held-key map (a hash insert
  and remove per press; no allocation).

## Permission records and the bundled pass: accepted leftovers (round 7 Part C, final review 2026-11-20)

- **Status:** Low, not scheduled. Both fail safe.
- The exact-shipped-package check runs on the resolved folder: a link at the bundled location that points at another
  folder holding exactly the shipped bytes still passes (no other code can be exempted by it).
- A package enabled and recorded in this build, then switched off and on again in 0.6.0 with wider grants installed
  in between, is blocked until re-enabled here instead of being adopted once.
- Waypoints: a player who clears and re-pins the identical point within one game step is not seen as the new owner;
  the one hook line it would take in the native waypoint setter is in the round 7C report (for the upstream-seam round).

## Mods & Addons window: look and reach (owner, 2026-11-20)

- **Status:** Wanted, not scheduled; needs a 5-line "what you'll see" first. The window is busy with text and does not
  look good; revisit its layout. It is also slow to reach while testing (Options, the Enhanced page, then the manager,
  every session): consider a Controls key for it and remembering that it was open.
- **Owner thought, 2026-11-20 (not a commitment):** rather than doing everything through the game's Options pages, one
  key (Shift+F10, which opens the addon manager from round 7 on; it was F10 until the 2026-11-25 sync, when F10 became the game's Settings) could open our own settings hub: the mod list, each mod's
  options, the Enhanced toggles and the mod keys in one window. Easier once `eqoa/1` is frozen; it could largely be a
  mod on top of the API. Decide together with the manager redesign.

## Left from the round 7 owner test (2026-11-20)

- **Mod keys on the loading screen and over the full map.** Ordinary mod keys (Stats Box, Map Search) still fire there,
  as in 0.6.0; only the Mods & Addons key is blocked in those states. Decide whether `addon_input_available()` should
  exclude them for every mod key (it changes when all mod keys work).
- **A player's own talk to a merchant within 3 s of a mod's sale** may not show the window on the first click. The fix
  is one upstream hook line in `Play::interact_with_target` (quoted in the round 7C report), for the upstream-seam round.
- **Unreliable test:** `round65_fix4_item1_bags_window_costs_under_a_millisecond_a_frame` counts creature-list scans per
  second against the design cap of 4.0 and reads 4.1 to 4.2 under load (failed once on CI and once locally on
  2026-11-20, passes alone). Measure over an exact interval or allow the timer's granularity; do not raise the cap.
- **Why Shift+Space did not jump in the owner's 15163407 test is unexplained** (by the code it should have; a Forge
  UI suppressing keys is the builder's guess). The behaviour is explicit and tested from 5d58d9c5 on.

## Left from the round 7H recheck (Hydrogen, 2026-11-21)

- **LOW: accepting a group invite while the selection is on No queues two key presses [136, 134].** 134 is held in
  `key_queue` until the next HUD frame (`crates/eqoa/src/play.rs` around 7133-7156, `fork_ext/play.rs` 3178-3193). If a
  server packet replaces the box within that frame, 134 lands on the new box without re-checking `prompt_answerable`.
  Safe today (a new Yes/No box resets its selection to No; untagged OK-only boxes have no action). Hardening: clear the
  queued presses when a new box opens, via a fork hook rather than editing upstream's `dialog.rs`.

## Owner wishes from the A1 test (2026-11-23)

- **Status:** Deferred; recorded from the owner's play test of dev 49f8abcc.
- **Spellbook:** ability icons and a better layout.
- **Quest Journal:** links from a quest to the map (where the map knows the place).
- **Frames / buff numbers:** show what an effect does in numbers (e.g. Anthem of Conquest: +N to which stats) if the client or server provides it. Owner 2026-11-23: low effort; if no API change is needed it is a later mod change or a small PR. A short look at the data is in reports/eqoa-owner-fixes-2-2026-11-23 (lead's notes).
  Result (2026-11-23): the client gets no effect numbers: the buff list has spell id, icon, time and name only;
  0x00B1's effect id is visual; ability records carry a server description text and unused "parts". A mod can
  estimate it from `eqoa.player.stats()` (`current` before and after one buff lands; maximums, not HP/power) for
  the player's own stats only, unreliable when other things change at once. Reliable numbers need the server to
  send stat deltas with the buff (a later additive `Buff` field).
- **Cast bars:** a player-style unit-frame set (your own cast bar, your target's cast bar, target health, etc.), as in modern MMOs. The API capability exists (A3/A4); this is a mod, not an API change.
- **Target Frame demo:** its menu shows "Invite to group" for every target (fine for a demo; a real mod should offer only what applies).

## After the round 8 offer: API additions left from the gap check (2026-11-26)

- **Status:** Deferred by the owner (2026-11-26): the offer goes first, because one consistent API matters more than
  coverage. All of these are additive to `eqoa/1`. Source: the gap check of 2026-11-26 (eight items; three were built
  in batch 8) and the reviews of batch 8.
- **The World Map follows the player's Minimap Colors choices** (owner asked 2026-11-27): today the mod copies
  upstream's default colours. Needs an API read of the player's palette (Extended > Minimap Colors) and of the PvP
  flag (the game's green for a flagged player).
- **Colour choices for players, pets and creatures on both maps** (owner asked 2026-11-27): upstream's Minimap Colors
  covers friendly NPC roles only; the rest use the game's fixed name colours. The API would hand the colour table to
  mods, and a mod would offer the choices.
- **Meters:** a read-only `eqoa.combat.dps_meter()`, and the server-fed group meter.
- **Effects on others:** a target's or creature's debuffs, and the effects on each group member.
- **Focus:** `eqoa.focus.get()` and a `focus` element for `eqoa.hud.hide`.
- **Inspect data:** on auction lots and on a mercenary's gear and spells; stats on recipes; `actions.craft_create(recipe)`.
- **Abilities and bars:** the autocast state of an ability; the details of a weapon-set bar slot.
- **ImGui names:** `ImGuiCol` and four other flag families are used by mods but are not in the documented surface.
- **One way to name a pack item:** four exist today.
- **Recorded creature locations as a shared read**, so the World Map's search box can list what Map Search has
  recorded (today it finds live creatures and group members only).
- **A cross-mod "is that window open" read:** Bags cannot come back by itself when the Auction window closes.
- **Stat figures without the player's own Stat Window setting:** `player.stats().figures` arrive while that setting is
  on or while a mod hides the Stat Window; anything wider needs an upstream line.
- **Move, look and button presses** wait on upstream.
- **A "send" choice in the crash offer:** since batch 8 no crash report is sent to a game server at all; offering it
  with an explicit yes per report is a separate, owner-approved piece of work.
- **Upstream's Target of Target bar and Faction Tracker (sync 2026-11-26, review M3, L1):** a `eqoa.hud.hide`
  element for each and their rectangles in `eqoa.hud.layout().pieces`. Until then upstream's Target of Target bar
  hides with the target frame, its Faction Tracker always shows, and a player who turns on both upstream's bar
  (HUD Extras) and the Target of Target mod sees two bars. The fork's own Enhanced row "Target of Target" has the
  same label as upstream's HUD Extras row.
- **Round Minimap on Night Chain 4 (sync 2026-11-26, review L2):** the mod's hover names still show there while
  upstream's minimap is blank; and a scene or GPU pixel test for the World Map mod's Night 4 cover and for the
  map's dot of an inactive group member (both are tested below the draw call only).

## Mod work approved or asked for, not built (2026-11-26)

- **Bag Viewer: "Open merchants and bankers in Bag Viewer"** (spec approved by the owner 2026-11-25). One option, off
  by default. On: talking to a merchant opens Bag Viewer on a Merchant tab (wares and prices left, bags right; a click
  buys one, stackables ask how many with the total, what you cannot afford is greyed; right-click or Sell (N) sells
  with the price shown and the API's sell question; Esc or walking away closes); talking to a banker opens the Bank
  tab (a click moves an item across, stacks ask how many, a tunar row). The Bank tab stays readable anywhere; the
  Merchant tab exists only while a merchant is open. Option off, or Bag Viewer off: the game's windows as today.
- **Quest turn-in that speeds through every step** and stops only at a real choice (later: the player picks a path and
  it follows). Needs a five-line "what you will see" for the owner first: today's match rule fits almost no real quest,
  and a stale Turn-in Helper beside Dialogs fights it for the same window.
- **A quest guide for this server; the World Map highlighting creatures a quest needs; an XP-camp flag on the World Map.**
- **Auto-repair's chat line.**
- **A plain-words message when two mods want the same window** (today: "response_dialog window is already taken over
  by eqoa_dialogs" in the log).
- **Mods that drift at a HUD Scale other than 100%:** checked for Mana Bars, Target of Target and the Target Frame
  demo in batch 8; any other mod drawing beside a game HUD piece should follow `hud.layout()` the same way.
- **A once-only notice that Mods & Addons moved from F10 to Shift+F10** (today a log line only).

## Mod project (later; the bundled mods are proofs of the API, owner 2026-11-25)

- One shared look for all mods (see "Match our UI to the EQOA Parser style").
- Bag Viewer: move the switches at the bottom of the window into a settings pane.
- Player, group and compass frames; a quest tracker; ability bars; controller support in the showcase mods.
- Name Plates: line of sight.
- An in-game meter mod that matches the owner's external parser (LoopRook/EQOA-Parser). The API already supplies most
  single-client figures; it needs the meter reads listed above.
- A sharper World Map picture (detail tiles are 3.9 units per texel).
- 48 of the 132 `eqoa.actions` have no bundled mod using them (all 17 mercenary, fishing and dye actions; four raid
  and master-loot actions): each wants at least a small example before it is called proven.

## Open technical questions after batch 8 (2026-11-26)

- **Hit tags from the owner's server:** the pet and mercenary tag-byte reading behind `combat.hit`'s `by_player`,
  `from_merc` and `attribution` has not been checked against a capture from that server. The owner's test with a
  mercenary out is the practical check.
- **Local telemetry for the external parser** (loopback only): heal and damage signs are inverted in
  `fork_ext/play.rs` `snapshot()` and its own test asserts the inversion. The owner says the parser is not needed.
- **Three render GPU pixel tests** (`ao.rs` twice, `particles16.rs`) fail on the lead's graphics card and are untouched
  by the fork: check them on plain upstream before calling any local run clean (see "Upstream AO tests fail on GPUs").
- **Upstream's updater** gives up after 120 seconds and deletes the partial download; inside a VPN the download can be
  too slow to finish. Resuming instead of deleting is upstream's code: a candidate for the offer or a later PR.
- **"Mods stopped responding" once on 2026-11-25**, cured by logging in again; cause unknown; nothing in the log.
- **A hidden HUD piece can show for a moment** before the first health update, because a mod's hides publish once the
  HUD is ready.
- **Frame-rate history:** the fixed-spot scene read 61.1 fps (batch 6), 50.9 (batch 7) and 68.0 (batch 8) in single
  runs on a busy PC. Repeat on an idle PC before quoting a number.
- **Controls not run red in batch 8:** the crash-report consent tests, most of the review-fix tests and the mod pass
  (in-test mutations only); there is no end-to-end test of Delete through the crash offer's dialog.

## From the owner's test of batch 8 on the PC interface (2026-11-26)

- **Status:** Deferred by the owner as mod work for later, except where noted. What he asked to be fixed now (Round
  Minimap scale on the PC interface, the Stat Window key opening the Stats Box, bank marks surviving a withdrawal,
  Support in the Start Menu mod, a Name Plates error, paste into the Chat line if small) is on `fix/owner-1126c`.
- **Target of Target does nothing on the PC interface** (it works on Controller). Mod work.
- **Mana Bars take about a second to show for a new target**, and neither Mana Bars nor Target of Target matches the
  look the mods are meant to share.
- **Effects window:** better as an icon strip, still "not great" and not in the shared look; "really unwieldy and not
  like a modern buff bar". An effect's hover text often says the game does not say what it does (seen on a mercenary's
  abilities; the player's own may be described). See "Frames / buff numbers" above: the client gets no effect numbers.
  Whether mod windows should collapse into drop-downs is a question for the mod project.
- **Hitches when coming back to the game window (lead, from his log):** sixteen frames of 150 to 600 ms in thirty
  minutes, logged as `slow: hud: mods took ... ms`, most of them on the first frames after the window comes to the
  front, several with "a background write was running", one inside `eqoa_map_search`'s events. Average frame rate on
  the PC interface was fine by his eye; the log holds no average. Wants a measurement (which mod, what it does on the
  first frame in front) before a fix.
- **Dev builds carry no version of their own** (owner: "we really should have been versioning all of these dev
  builds"): every dev build says 0.7.0 plus its commit. A counter or a `-dev.N` suffix in the build id would make "which
  build did you test" a one-word answer. Small; touches the Nightly workflow and the build stamp.

## Map Search and Creature Journal: hold less in memory (owner, 2026-11-26)

- **Status:** Deferred to the mod project. The room was raised instead for now: a mod with `"large_data"` has 256 MiB
  of Lua memory since 2026-11-26 (was 128 MiB; an ordinary mod still has 8 MiB).
- **Why:** with a 10,000-name, 80,000-spawn journal (the test's worst case) Map Search peaks at about 101 to 111 MB and
  the Creature Journal at 93 to 102 MB, about twice what each really holds, because the garbage of a load or an index
  build counts until Lua's collector runs. A fix on 2026-11-26 briefly took Map Search to 131 MB of 134 MB and the
  test failed now and then.
- **What to do:** keep spawn rows as plain arrays (not one table per spawn), make the index text cheaper, load the
  journal lazily when the panel first opens, and let go of the catalogue when the panel has been closed for a while.
  The test `round4b_fix_busy_world_max_journal_cost_and_incremental_queries` keeps its guard at 90% of the OLD 128 MiB
  on purpose, so the mods do not grow into the new room; lower that guard as the mods get leaner.
- **Not measured:** the owner's real journal in play.

## Check upstream's new features against the API for gaps (owner, 2026-11-27)

Check each of upstream's new features (Bestiary, bank bags, Faction Tracker, creature loot chart, Target of
Target, Bags + Compare Gear) against `eqoa/1`: can a mod read and do what the feature shows? List the gaps;
additions come after the offer to upstream.

## API tiers: let the scan follow kept and copied values (lead, 2026-11-27)

`tools/api_tiers.py` proves a record field only on a value it can follow from the API call. Fields our mods
really read stay provisional when the value is kept in a table first (`state.guild = eqoa.guild.get()`), copied
into a sorted list, passed to a function that also takes other tables, returned from a function with two
returns, or defaulted after `or` (30 examples are in the builder's report of 2026-11-27,
section 7; `Ability.name`, `Friend.name`, `ChatMessage.text`, `Guild.motd` among them). Extending the scan only
promotes names (no `--allow-demote`); each new pattern needs a negative control, and the reviewer's 128 names
must stay provisional.

A second review (2026-11-27) found no falsely proven name, and five loose bindings in `tools/api_tiers_flow.py`
that prove nothing wrong today but could with a future mod; fix them in the same round, each with a negative
control: the second result of an `eqoa.imgui.*` call is typed as `eqoa.Error` (it is for data calls only); a
function name defined once in a mod is tied to any `x.name(...)` or `x:name(...)` call in another file; call
arguments are typed before `and`, `or` and comparisons are looked at (`f(c == nil)` passes `c`'s record);
`pairs(x)` and `x[k]` keep the record type of a single record; `producer() or {}` keeps the record type.
