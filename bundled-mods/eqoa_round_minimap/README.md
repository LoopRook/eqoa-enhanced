# EQOA Round Minimap

Enabled by default. It hides the game's square minimap and shows a round one in the same
place and size, with a ring. Move HUD moves it. N toggles it (the game's own setting) and
the wheel zooms it. Hovering a creature, player or pet shows its name, level, kind and
distance; hovering a camp, coach or quest mark the minimap draws shows its name (not towns; camps and coaches follow Map Markers). The whole minimap goes while a full map is up (the game's own, or the World Map mod's window: any other mod that is hiding the minimap takes it away) and returns when the map closes. Disable the mod and
the game's square minimap returns untouched.

The picture is the game's own minimap (terrain, indoor view, camps, coaches, quest marks,
waypoints and group), taken through `eqoa.map.view` and cut to a circle.

Limits: marks the square picture puts in its corners are cut by the circle. The player's
waypoint and the group waypoint get a rim arrow outside the ring there; a quest step
pointing diagonally off the edge is not shown. Clicking the minimap does not open the map
(press M).

Look ported from EQOA Enhanced map/mini.rs, AGPL-3.0-or-later. Space Mono: OFL.txt.
