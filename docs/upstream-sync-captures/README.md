# Upstream synchronization captures

Pinned upstream: 8d90350. Enhanced baseline: 8aeaf91.

The post-merge minimap.png, mob-hover.png and spawn-search.png are byte-identical
to matching refined-* baseline files in ../map-parity-captures/. They use the
same offline synthetic NPC fixture, not a live-server spawn check.

world-before.png and world-after.png use a locally owned disc, TUNARIA.ESF walk
0, EQOA_SPAWN=25300,15200, and a 1000 ms stationary script. The post-merge run
reports one queued/drawn character shadow, four triangles. This does not verify
every indoor portal or server case; the upstream deterministic tests cover their
fixtures. The initial world-shadows.png is an earlier ocean smoke test.

manifest.json records the original capture binary hash and a source audit:
all newly added upstream files match 8d90350; listed enhanced files match 8aeaf91.
The capture binary predates the hygiene scope correction that leaves the GM
keyboard proposal unapplied; that correction cannot affect these map renders.
Final hygiene captures/build provenance will be recorded separately.
