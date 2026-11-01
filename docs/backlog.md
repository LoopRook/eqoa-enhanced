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

## Save problem reports to the Desktop

- **Status:** Deferred until the Wereoxxs dev-build gate is met.
- **Requested:** 2026-10-30.
- **Goal:** Players who are not technical can hand the owner everything needed to diagnose a problem, with no server. F3 (or the HUD's Report a bug) and the next-start crash offer keep upstream's dialog (screenshot, "what happened" note, log tail, client state), but **Send becomes "Save to Desktop"**: one `EQOA-Enhanced-report-<date>.zip` with the note, screenshot, `eqoa.log`, `eqoa.prev.log`, `mods/addons_error.log`, any waiting crash files and the client state, then open the Desktop with the file selected. The player shares that one file however they like.
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
- **Shared community journal (plan):** 1) native journal (offline; file-compatible with world_map.lua). 2) the client downloads a community journal from our service, the way it checks for updates, and merges it in. 3) the client uploads only new sightings (name, zone, level, position, entity id) in small batches with a random install id; no account, character name or paths. Service-side: schema and range checks (levels 1-60, positions inside the zone), size caps, per-install and per-IP rate limits. An entry enters the public list only when at least 2 different installs report it, which guards against poisoning. The service is labelled EQOA Enhanced and never involves upstream. The client carries no write secret, because anything shipped in the exe can be extracted.
- **Decided (2026-10-31):** no disposition or con colour in shared data. Both depend on the viewer (faction/alignment, and the viewer's level), so shared values would mislead. Con colour is computed locally from the recorded level. Telling Wereoxx is optional and not planned. Release notes should mention the automatic sharing (recommended; the owner has not objected).
- **Acceptance (draft):** seeing a creature records it once; walking past the same spawn again adds nothing; a merge adds only new entries; search finds a journal-only creature; a round-trip with a file written by world_map.lua loads unchanged.

## Match our UI to the EQOA Parser style (owner, 2026-11-01)

- **Request:** the client's own UI (map search, Addon Manager, profiles and so on) was meant to look like the owner's EQOA Parser (`C:\Users\hocka\Github Projects\EQOA Parser\`), which took its rules and inspiration from Track Level Companion (`C:\Users\hocka\Github Projects\Track Level Companion\`). Both are on the owner's GitHub.
- **Status:** deferred, not urgent. Next step: a read-only scout extracts the style rules (palette, fonts, spacing, components) from both projects into a short style guide, then one restyle pass over the fork-owned UI.
