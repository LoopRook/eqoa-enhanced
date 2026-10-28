# Native map before / after

These are deterministic native Rust/WGPU map renders using a locally owned
Frontiers disc, not HTML mockups or live server sessions. Minimap-only shots have
a black background because mapshot does not render the gameplay scene behind it.
They do not prove live group movement, spawning, zoning or network submissions.

The baseline is the previous playable UI preview, SHA256
99f189e16f635b31408bf529693c0e6e60d036cac422c41a5565239a09856fc0.
The final executable hash and matching case parameters are in capture-manifest.json.
Both versions use the same catalog, terrain cache, player position and window size.

| Case | Before | After | Review |
|---|---|---|---|
| Search / coordinates, 1280x720 | [before](before-world-search.png) | [after](after-world-search.png) | Search below coordinates, label typography, XP badges and flat panels. |
| Overview, 1280x720 | [before](before-world-overview.png) | [after](after-world-overview.png) | Town labels and compact distant camp marks. |
| Guide, 1920x1080 | [before](before-minimap-guide-wide.png) | [after](after-minimap-guide-wide.png) | Square replaced by a physical-pixel circle; historical NPC guidance at the rim. |
| Search, 720x1280 | [before](before-minimap-search-portrait.png) | [after](after-minimap-search-portrait.png) | Circular minimap and separate unmasked search panel at a tall aspect ratio. |

Additional final-build evidence: [personal waypoint guide](after-waypoint-guide.png)
and [inline Suggest](after-suggest.png). Suggest was opened offline; nothing was
submitted to the community endpoint. The waypoint is at (25800,16200), player
(25300,15200), window 1920x1080.

## Reproduce

Set EQOA_ISO to a locally owned disc, EQOA_CONFIG to a scratch profile and
EQOA_MAP_DATABASE to the pinned external catalog. Build the native client, then:

    eqoa.exe mapshot TUNARIA.ESF output.png 25300 15200 0 5 15

EQOA_SHOT_SIZE selects width x height. EQOA_MAP_QUERY supplies search text;
EQOA_MAP_TRACK=1 selects the first result; EQOA_MAP_MINI=1 captures the minimap.
EQOA_MAP_HOVER=x,z selects a map hover coordinate; EQOA_MAP_COORDS=1 exercises
the pointer-coordinate line. EQOA_MAP_WAYPOINT=x,z plus EQOA_MAP_TRACK_PIN=1
selects personal-pin guidance. EQOA_MAP_SUGGEST_FORM=1 opens the inline form.

Remove unused environment variables between cases; an empty value still enables
presence-based flags. The old preview already exposed the hover-coordinate line;
the final capture additionally exercises the explicit coordinate flag. Fixtures
supply no live actors or group roster. Never commit disc/config/account files.

## Font clarity follow-up

Compare [text-before.png](text-before.png) with [text-after.png](text-after.png).
Both use the same Northern Ro camp tooltip, player (24109,16574), hover
(24123.5,16547.0), scale 5, and 1280x720 window. Label/card geometry and wrapping
remain the same; the latter keeps full font atlas detail instead of enlarging
a reduced bitmap. These supersede the first milestone's label quality.
Executable/image hashes and parameters are in text-manifest.json.
