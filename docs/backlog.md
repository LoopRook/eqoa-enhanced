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
