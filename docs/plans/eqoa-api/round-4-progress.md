# Part A ledger ? docs/plans/eqoa-api/round-4-brief.md

- Brief copied and backlog recorded: 68527d9.
- Spec addendum: fixed owner decisions, existing map rows retained.
- Preflight: view texture consumes existing texture ownership; guide consumes entity events; pointer/layout feed the later mods. No conflicting interfaces found yet.
- Next: item 1 native state/events, item 2 marker sources, item 3 views, then items 4?10 in order.
- Item 1: Play map geometry/state ? native snapshot; map.changed only on changes; unavailable fields stay nil. Regression map_state_comes_from_play_and_changed_event_has_negative_control: observed RED without binding, GREEN (map-filter run: 38 pass, only the pending item 2 test fails). The initial fixture borrow failure was corrected before the meaningful RED.
- Footprint: map/mod.rs +3 adapter declaration; total measured divergence +48/-7 fits the existing +70/-10 budget after the syncs. No budget increase is needed.
