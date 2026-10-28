# Map and Tracking Mockup

Native search, guidance and UI treatment are now implemented. Start with ../../docs/MAP-RELEASE-HANDOFF.md for the delivered milestone; the following study predates the native build.

Open `index.html` in a browser. This is a standalone, dependency-free interaction
study of an in-game map/minimap, not an installed feature or a planned web client.
No parser or native game code is changed.

## Try It

- In World Map, search NPCs by name/level, inspect known spawn areas, and Track.
- Close the map or choose the preview's In-game button. Guidance stays inside
  the minimap; there is no permanent tracking card or nearby notification.
- Use the minimap search button. NPCs, Group, and Places share the temporary panel.
  Choosing an eligible result starts guidance and closes the panel.
- Group has sample members Aerin/Bran and an unavailable-position example Sable.
  Unavailable members cannot be guided to. Use the rim's cross to stop tracking.
- Hover/focus the minimap for target details. The Sample live NPCs checkbox is a
  preview-only simulation: live highlights and recorded guidance coexist.
- Switch Parser theme / Map study theme. Search, selection, guidance, and marker
  state remain intact; the same controls/behavior consume different theme tokens.

## Presentation and Data

The map dominates; a compact full-map search panel and transient minimap search
serve the same guide-target model. A 220px circular minimap preserves HUD space.
Group, quest, and coach symbols retain their semantic colors across skins.

The Parser skin references the existing parser's `ui/web/style.css` and
`docs/research/ui-overhaul.md`: near-black surfaces, thin borders, bracketed actions,
monospace controls, and a vermilion accent. It uses installed monospace fonts with
Consolas fallback and does not load or modify parser assets. The Map Study skin
retains the earlier earth/brass style. See `../../docs/MAP-THEMING.md` for the native
theme contract; a native theme-pack loader is not implemented by this mockup.

NPC/group records, levels, coordinates, distances, spawn areas, and scenery are
illustrative. The regional background is schematic, not geographically calibrated
game art or a Forge asset. Group movement is not connected to a live server.
Browser preview controls are not proposed permanent game controls.

## Validation

Node syntax check and browser interaction checks cover NPC quick search,
group selection, unavailable-position controls, guide-state retention across skin
changes, independent live/recorded marker layers, search closing after selection,
and the absence of the old tracking card. Desktop and narrow-screen layout are
reviewed separately; these checks do not establish native game functionality.

## Historical Next Native Work

Extend the map's context with stable group identity and position validity, then
wire the shared guide target to NPC/recorded-location/group rendering. Add a
presentation boundary for theme tokens/assets; preserve quests, coaches,
community pins, and personal/group waypoints. Parser implementation stays with
the other agent.
