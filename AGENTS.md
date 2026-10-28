# Repository Guidelines

## Project Structure & Module Organization

This Rust 2024 workspace implements a native EQOA: Frontiers client. Read `docs/MODDING-AND-PARSER-ROADMAP.md` for enhanced-branch goals, parser failures, and Forge map parity requirements. Read docs/MAP-RELEASE-HANDOFF.md before extending the map.

- `crates/eqoa/`: client binary, gameplay integration, HUD, pregame screens, and platform code.
- `crates/vi/` and `crates/ee/`: game logic and Emotion Engine arithmetic.
- `crates/iso/`, `esf/`, `dnas/`: disc access, asset formats, and module unpacking.
- `crates/render/`, `snd/`, `drdp/`, `net/`: rendering, audio, transport, and client protocol. WGSL shaders live in `render/src/`.
- Tests live beside modules and in each crate's `tests/` directory. `web/` holds the browser client; `tools/` contains generators; `mods/` contains addon manifests.

## Build, Test, and Development Commands

From the repository root:

- `cargo build --release --locked -p eqoa`: build the native client.
- `cargo run --release -p eqoa --`: launch the game and disc picker.
- `cargo test --locked --workspace`: run the workspace suite, matching Linux CI.
- `cargo test --release -p vi`: run game-logic and available oracle tests.
- `cargo test -p eqoa <test_name>`: run a focused regression test.
- `sh web/build.sh`: build browser assets into `web/dist/`; requires the WASM target and the matching `wasm-bindgen-cli` version documented in README.

Linux builds require `libasound2-dev`, `libudev-dev`, and `pkg-config`.

## Coding Style & Naming Conventions

Use four-space indentation, `snake_case` functions/modules, `PascalCase` types, and `SCREAMING_SNAKE_CASE` constants. Match surrounding formatting; use rustfmt on touched code. No custom formatter or Clippy gate is configured.

Game-logic arithmetic and comparisons use `ee::*`. Preserve `vi`'s `#[repr(C)]` layouts for oracle checks. Read `CLAUDE.md` before changing ported systems; README describes their reference checks and architecture.

## Testing Guidelines

Use Rust's built-in `#[test]` framework with descriptive behavior-based names. Add regression tests with negative controls for new ports. Disc, oracle, and GPU tests can skip when prerequisites are missing; report these skips. There is no configured coverage percentage. Compare renderer changes using deterministic `shot` captures.

## Commit & Pull Request Guidelines

Commits mix imperative summaries and scoped prefixes such as `fix(launcher):` and `feat(ui, mods):`. Keep commits focused. PRs should explain changes, link relevant issues, list validation and skips, and include screenshots for visual changes. Update `UPDATE-NOTES.md` for player-visible changes. Push complete, tested changes: program changes on `master` trigger CI releases consumed by the updater.

## Game Data & Configuration

Keep disc images, decrypted modules, server patches, and credentials untracked. Use `run/eqoa.iso` or `EQOA_ISO` for a locally owned disc. Test reports and patching against local services with a scratch `EQOA_CONFIG` directory.
