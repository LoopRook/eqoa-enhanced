# Map UI Theming Contract

## Requirement

The user wants future community themes. Keep presentation replaceable while NPC
search, group guidance, quests, coaches, and waypoint behavior stay independent.
This applies to the native game's map/HUD renderer, not a web interface embedded
in the game. The HTML study is a disposable interaction preview.

## Boundaries

- Data: NPC catalog, live entity IDs/positions, group roster, quest/coach markers.
- Behavior: search, selection, guide target, input focus, persistence, and routing.
- Presentation: theme tokens/assets consumed by map controls and marker rendering.

Shortcut labels and controller glyphs come from the current input bindings, not
theme files; a theme must not hard-code M, N, or other default controls.

Switching a theme must preserve search text, filters, selection, tracked target,
zoom, waypoints, and group state. Themes never supply positions or run gameplay
commands. Different themes render the same model and actions.

## Supported Presentation Surface

Start with semantic tokens for:

- Surfaces: background, panel, input, selected, hover, and focus.
- Content: primary/secondary text and action colors.
- Frames: outlines, thickness, corner radius, padding, and supported density/scale.
- Fonts: UI text and map labels, with a readable bundled fallback.
- Assets: search/stop/zoom controls, minimap frame, coach and quest icon slots.
- Map marks: recorded locations, live matches, selected group member, personal
  waypoint, group waypoint, and quest states. Preserve semantic identity across
  themes; pair colors with symbols/labels rather than relying on color alone.

Avoid skin-specific branches in search/tracking code. Layout uses supported
anchors and metrics rather than arbitrary executable templates. A broad plugin
API and a full visual editor are separate future work.

## Implemented Native Treatment

`crates/eqoa/src/map/theme.rs` supplies the native map's surface, input, border,
focus, selection, text and semantic marker colors. Search, map controls,
minimap controls and the inline Suggest form consume these helpers. The embedded
Space Mono font is rasterized once into the native font atlas at three times the
logical size. Its SIL OFL notice is in `crates/eqoa/resources/map-ui/OFL.txt`.

`EQOA_MAP_FONT` can name a local TTF override; unreadable/invalid fonts fall back
to embedded Space Mono. Map controls, labels, tooltips and XP camp badges use the font override. The
existing game UI retains its normal font and behavior. Runtime theme manifests
and selectable color packs remain future work.

Current source revisions, native adaptations and asset licenses are recorded in MAP-RELEASE-HANDOFF.md.

## Future Native Theme Pack

Proposed convention, not implemented yet:

```text
themes/<theme-id>/
  theme.json
  icons/
  fonts/
  LICENSES.txt
```

The versioned manifest selects supported tokens, fonts, and icon/frame assets.
Resolve assets within the pack; use defaults for missing/invalid fields and
unsupported schema versions. A failed theme must leave the map usable and retain
the selected guide target. Cache resolved assets outside the draw loop.

User HUD position/size preferences stay separate from theme defaults. Map
background art and world-coordinate calibration are also separate: changing a
button skin must never move an NPC marker or invalidate the terrain cache.

## Design References

Primary reference: Track Level Companion, CLAUDE.md section 6.E and the Nothing tokens in src/index.css. Apply flat #111111/#1A1A1A surfaces, #333333 borders, #E8E8E8 text, #999999 secondary text and #D71921 accent; avoid drop shadows. The circular minimap uses a shared GPU mask and physical-pixel border. Search anchors below measured coordinate/progress chrome, not a fixed y position.

Read-only secondary references from the existing EQOA Parser checkout:

- `eqoa_parser/ui/web/style.css`: near-black dark surfaces, thin neutral borders,
  `#D71921` action accent, and Space Mono controls.
- `docs/research/ui-overhaul.md`: token definitions, bracketed action labels,
  compact density, reduced motion, and font rationale.

The map preview includes Parser and Map Study skins. Both supply centralized CSS
tokens to shared controls and tracking logic. The parser-inspired preview uses a
local monospace fallback if Space Mono is unavailable; it does not load or modify
parser assets. This demonstrates the separation, not a native theme loader.

## Verification Targets

Switch both skins while tracking a group member: identity, guidance line, query,
and selection remain unchanged. Check quest/coach/group visibility and input
focus in each skin. Test scaling and narrow windows. Before shipping native
themes, additionally test missing fonts/icons, invalid manifests, fallback
behavior, and persistence across restart.
