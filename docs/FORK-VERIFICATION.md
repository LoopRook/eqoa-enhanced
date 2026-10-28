# Fork hygiene verification — October 28, 2026

Upstream pinned and rechecked remotely: 8d9035096a2deb447854d5fcff4ef25225f80fd1.
Merge checkpoint: 1bfe228, with parents enhanced 8aeaf91 and upstream 8d90350.
The cleanup follows that merge without rewriting shared history.

## Commands and output

Native Windows release, after the final source edit:

    cargo build --release --locked -p eqoa
    Finished `release` profile [optimized] target(s) in 1m 38s

Native executable SHA256:

    5555caebd557dc923cdcadf3910f39ff21a259832fc92627855b0698ad33912c

Actual browser script, using Rust's wasm32-unknown-unknown target and matching
wasm-bindgen-cli 0.2.129 (Git sh on Windows):

    sh web/build.sh
    Finished `release` profile [optimized] target(s) in 1m 19s

Exit 0; generated versioned JS/WASM assets and included report-policy.js. The page
build runs wasm-bindgen and the native webicons generator, not just cargo check.
This is compile/package evidence, not an interactive browser gameplay test.

Workspace, scratch EQOA_CONFIG with default absent disc/oracle fixtures:

    cargo test --locked --workspace -- --nocapture --test-threads=1
    91 test/doc-test summaries: 1224 reported passed; 0 failed; 2 ignored

168 Cargo-ok results omitted fixture checks. Every omitted test and both ignored
benchmarks are named in [FORK-TEST-SKIPS.md](FORK-TEST-SKIPS.md). The other 1,056
results had no identified fixture omission. Renderer GPU checks did execute.
Afterward, a test-only options lookup was adjusted to keep one textual hook
reference; its exact focused test passed. No production behavior changed.

Additional targeted checks:

    cargo test --locked -p eqoa hud:: -- --nocapture
    7 passed (the new hook test failed before implementation)
    cargo test --locked -p eqoa fork_
    13 passed (adapters, services and HUD; adapter negative controls verified red)
    cargo test --locked -p eqoa options::
    15 passed
    cargo test --locked -p eqoa settings::
    39 passed, including Unicode device-name round trip
    cargo test --locked -p eqoa play::commands::an_own_hit_reaches_the_swing_timer -- --exact
    1 passed
    cargo test --locked -p eqoa options::tests::the_extended_pages -- --exact
    1 passed after final test-only lookup correction
    python -X utf8 tools/test_parser_launch.py
    9 tests OK
    python -X utf8 tools/test_fork_footprint.py
    11 tests OK
    node web/report-policy.test.mjs
    destination policy and default F3/API/crash/previous-session denial harness OK
    python -X utf8 tools/check_fork_footprint.py
    31 shared paths within reviewed budgets; 12 hooks exactly once; PASS

Launcher tests execute real fixture children, including paths with spaces,
manifest argv/cwd/PATH resolution and Windows GetConsoleWindow()==0. They do not
launch or modify the separate real parser. Its __main__.py and --client argument
were inspected read-only. Native report tests use explicit local fixture servers;
no report is sent upstream. Python source compilation and Git diff checks pass.

## Preserved map and upstream work

Three fresh native mapshot captures match the merged checkpoint and pre-merge
enhanced baseline byte for byte: circular minimap, NPC-dot hover and spawn search.
Parameters and SHA256 values are in
[upstream-sync-captures/hygiene-manifest.json](upstream-sync-captures/hygiene-manifest.json).
The referenced images remain the existing minimap.png, mob-hover.png and
spawn-search.png. Duplicate identical images are not added.

The captures use the locally owned disc, pinned Forge catalog and synthetic
Water Rat level 7. They are offline presentation checks, not live spawn evidence.
All restored map and quest implementation files remain unchanged by this cleanup.
Initial upstream source/capture audit remains in UPSTREAM-SYNC.md and the original
manifest; its old mods.rs/telemetry.rs entries describe the pre-extraction snapshot.

## Animation branch and limits

fix-animation-oob-panic is prepared at 231f0e9 directly atop 8d90350. It is not pushed
or submitted. The original b0eb7f5 branch state remains in the backup ref. Its
release VI run reported 80 ok, with 19 exercised and 61 fixture omissions. Enhanced
retains the existing Anim::frame fallback and adds the same empty-clip guards and
character regression. Upstream's negative-clock nearest-end behavior is preserved.

Live moving-group guidance, zoning/reconnect, despawns, fishing/GM permissions and
all indoor shadow cases still require player/server checks. The map playtest and
remaining full-parity items stay in MAP-PLAYTEST.md and MAP-RELEASE-HANDOFF.md.
No parser overhaul, license change, public release, master push or PR submission
is included in this pass.
