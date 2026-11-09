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

- **Request:** the client's own UI (map search, Addon Manager, profiles and so on) was meant to look like the owner's EQOA Parser (`C:\Users\hocka\Github Projects\EQOA Parser\`), which took its rules and inspiration from Track Level Companion (`C:\Users\hocka\Github Projects\Track Level Companion\`). Both are on the owner's GitHub.
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
