# Developer Tools

This document describes the three Python developer helper tools located under `tools/`.

## 1. Branch Gates Runner (`tools/run_gates.py`)

Runs the standard development gates in serial order, managing the shared Cargo build lock, streaming step logs, and emitting a summary Markdown report.

### Command

```bash
python tools/run_gates.py [options]
```

### Steps

1. `round`: `cargo test --locked -p eqoa --bin eqoa -- <filters> --test-threads=1 --nocapture` (filters from `--filter`; step skipped when no filter is provided).
2. `addons`: `cargo test --locked -p eqoa --bin eqoa fork_ext::addons:: -- --test-threads=1 --nocapture`.
3. `full`: `cargo test --locked -p eqoa --bin eqoa -- --test-threads=1 --nocapture --skip round4_ --skip frame_time --skip benchmark --skip scrollback_10000 --skip measure` (extended via `--extra-skip`).
4. `wasm`: `cargo check --locked -p eqoa --target wasm32-unknown-unknown`.
5. `footprint`: `python tools/check_fork_footprint.py --base <reviewed_base>`.
6. `paths`: `python -X utf8 tools/check_no_local_paths.py`.
7. `player-release`: `python -X utf8 tools/test_player_release.py`.
8. `release` (only with `--release`): `cargo build --locked --release -p eqoa` with `RUSTFLAGS=-C target-feature=+crt-static`.

### Options

- `--only <steps>`: Comma-separated list of steps to run (e.g. `--only footprint,paths,player-release`).
- `--skip-step <steps>`: Comma-separated list of step names to skip.
- `--filter <filter ...>`: Test name filters passed to the `round` test step.
- `--extra-skip <test>`: Additional test names or prefixes to skip in the `full` step (can be specified multiple times).
- `--release`: Run the final release build step with static CRT.
- `--dry-run`: Print commands and environment without executing anything.
- `--lock-wait <min>`: Lock wait timeout in minutes (default: 60).
- `--timeout <min>`: Per-step execution timeout in minutes (default: 60).
- `--root <path>`: Repository root path (default: current working directory).

---

## 2. Bundled Mod Manifest Updater (`tools/update_bundled_manifest.py`)

Maintains exact SHA-256 checksums and file lists for bundled mods across three files:
- `bundled-mods/SOURCE.txt`
- `tools/build_player_release.py` (`BUNDLED_MOD_FILES`)
- `tools/test_player_release.py` (`BUNDLED_MOD_FILES`)

### Command

```bash
python tools/update_bundled_manifest.py [options]
```

### Options

- `--check`: Verify manifests without modifying files on disk. Exits with code `1` and prints unified diffs if any manifest is out of date; exits `0` if up to date.
- `--root <path>`: Repository root path (default: current working directory).

---

## 3. Owner Test Packager (`tools/make_owner_test_package.py`)

Assembles a self-contained test deployment directory for owner playtesting. The resulting folder layout contains:
- `config/`: Copy of local player configuration (from `%APPDATA%/eqoa-rs`).
- `play/`: Extracted release build assets from `build_player_release.py`.
- `Play EQOA {name} Test.bat`: Standalone launcher targeting `config/` with updater disabled.
- `Play EQOA {name} Test (fps log).bat`: Launcher enabling `EQOA_PERF=1`.

### Command

```bash
python tools/make_owner_test_package.py --exe <path/to/release/eqoa.exe> [options]
```

### Options

- `--exe <path>`: Path to built release executable (required; this tool does not build).
- `--name <label>`: Package and launcher name (default: `Round5`). Used in folder name (`EQOA-Enhanced-{name}-Test`) and launcher files.
- `--channel <name>`: Release channel (default: `enhanced-dev`).
- `--output <path>`: Override package destination directory (default: `<Downloads>/EQOA-Enhanced-{name}-Test`).
- `--catalog <path>`: Optional local entity database JSON path.
- `--commit <hash>`: Optional Git commit assertion passed to `build_player_release.py`.
- `--version <ver>`: Optional version string assertion passed to `build_player_release.py`.
- `--force`: Replace `play/` and batch files in an existing output folder. Preserves existing `config/` unless `--fresh-config` is also specified.
- `--fresh-config`: Overwrite existing `config/` when `--force` is used.
- `--root <path>`: Repository root path.
