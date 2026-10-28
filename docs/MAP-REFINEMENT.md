# Native minimap refinement

Follow-up to the playable milestone and font correction. Parser work remains
owned by another agent.

## Intended behavior

- Replace the wide minimap Search control with a 20-unit vector magnifier.
  Preserve configurable Search/Stop bindings and keyboard/controller actions.
  Wheel zoom remains; no additional minimap zoom buttons. The yellow question
  mark remains a quest step, not a search affordance.
- Hover current scene dots for name, NPC/player kind, reported level when known,
  and distance. Pick the closest visible dot and reject the circular mask's
  invisible area. Static quest/coach/camp tooltips continue to work.
- Search says **Spawned nearby** for a currently observed living NPC and
  **Not detected (range/spawn unknown)** otherwise. It cannot report server-wide
  spawn status. Guide to a live match when available, falling back to recorded
  locations after observations disappear.
- Background focus must not connect a virtual controller when none exists;
  preserve a real controller at rest and genuine disconnect handling.

## Work and checks

Add failing presence, guidance, hover/picking, compact-control and controller-focus
regressions; implement against existing map/input paths; run native and renderer
checks; inspect deterministic native captures; document evidence; integrate locally
into enhanced and refresh both normal executables and the preview bundle.


## Delivered and verified

All six new/regression cases plus the complete native suite passed:
**911 passed, 2 existing FX benchmarks ignored**. The five initial checks were
watched fail before the implementation. Existing prerequisites can still skip
disc/oracle fixture checks as described in MAP-RELEASE-HANDOFF.md. A fresh source
review found no actionable correctness issues.

Native captures cover compact controls, the entity tooltip, both spawn states,
and portrait layout. See map-parity-captures/README.md. Their NPC is an explicit
offline fixture; live gameplay spawn/death/despawn and device unplug behavior
still need the MAP-PLAYTEST.md checks.

The controller defect was Windows Auto focus becoming Pick::Idle without any
physical controllers; that falsely connected the libpad port while unfocused,
then returned it to disconnected on focus. Background selection now stays
Pick::Nothing when no controller exists. A real resting controller stays
connected, and existing real-disconnect handling is preserved.

The yellow quest ? and existing full-map zoom controls remain. Only minimap
Search changed to a magnifier; mouse-wheel minimap zoom and saved bindings remain.
Guide captions are transient on hover, avoiding permanent redundant HUD text.
