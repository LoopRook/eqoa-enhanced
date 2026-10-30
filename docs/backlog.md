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
