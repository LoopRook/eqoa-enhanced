# Development

## Player delivery

Players should receive a prepared Windows ZIP, extract it and double-click
eqoa.exe (or the versioned preview EXE). They do not build from source or need
Python for the game. Keep build commands in developer documentation.
Start-EQOA.bat is a checkout convenience; Play Map Preview.cmd selects an isolated
profile and catalog for testing. Neither is a required player launcher.
The Python desktop launcher and external parser remain optional companion tools.

## Build and test

Use the Rust toolchain pinned by .github/workflows/dev.yml. From the workspace:

```powershell
cargo build --release --locked -p eqoa
cargo run --release --locked -p eqoa --
cargo test --locked --workspace
```

Linux additionally needs libasound2-dev, libudev-dev and pkg-config.
Supply your own disc through the picker, EQOA_ISO or run/eqoa.iso. Keep decrypted
CLIENT/SUPPORT modules, accounts, server patches and local databases untracked.
Use a scratch EQOA_CONFIG for tests touching saved state.

The normal BAT prefers target/release/eqoa.exe over root eqoa.exe. Broad historical
NPC search requires map-entities.json beside the executable or EQOA_MAP_DATABASE.
tools/build-map-preview.ps1 builds a complete bundle, verifies that catalog and
excludes saved accounts from its ZIP.

## Architecture and reference checks

The workspace owns native game structures; it does not embed a PS2 emulator.
Game logic and collision live in vi, EE arithmetic in ee, disc/assets in
iso/esf/dnas, rendering in render, audio in snd, transport/protocol in drdp/net,
and game integration/HUD/platform code in eqoa. Fishing and GM integration remain
separate play modules.

Port logic against the game's reference behavior using ee arithmetic and
comparisons. Preserve repr(C) layouts used by oracle tests. Rendering need not be
bit-exact, but changes must preserve pass order and should have deterministic
captures. Oracle and disc tests can return early when local prerequisites are
missing; report those skips, not just a successful test-process exit.

Map behavior and presentation boundaries are documented in MAP-THEMING.md and
MAP-RELEASE-HANDOFF.md. Key labels resolve saved bindings. Read UPSTREAM-SYNC.md
before another merge; preserve both upstream and enhanced intent rather than
resolving whole source files to one side.

## Browser and detailed reference

web/build.sh builds the browser client. Install the wasm32-unknown-unknown target
and wasm-bindgen-cli matching Cargo.lock. Browser execution also requires a locally
owned disc; it is read in the browser rather than uploaded.

The upstream README maintains the detailed per-system reference ledger, oracle
provenance and browser setup. Link that source instead of duplicating its large
tables in this fork:
[upstream technical reference](https://github.com/DabDavis/eqoa-rust-client-vanilla/blob/8d90350/README.md).
The local roadmap and map handoff describe the enhanced differences.
