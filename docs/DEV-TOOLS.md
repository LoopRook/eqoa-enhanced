# Developer Tools

This document describes the three Python developer helper tools located under `tools/`.

## Mod API reference (`eqoa mod-api`)

The native client can print the capabilities installed for an `eqoa/1` addon:

```powershell
eqoa.exe mod-api
eqoa.exe mod-api --format json
eqoa.exe mod-api --format md --out docs/modding/SUPPORT-TABLE.md
eqoa.exe mod-api --format json --out docs/modding/eqoa-1-surface.json
```

Markdown is the default. `--out` writes UTF-8 to the named file; without it the
reference goes to stdout. Invalid options, install failures and output errors
exit with code 1 and an error on stderr. The command installs a scratch native
addon and its ImGui, texture, font and sound adapters. It uses a headless graphics
device (required), creates no game window, and needs no disc, login or server.
It does not run the addon's entry script or call game actions.

Generate both committed files with a Windows release build, the published
platform; sound capabilities differ elsewhere. Build provenance uses the compiled
client version and `EQOA_BUILD_COMMIT` stamp (or explicitly says it is unstamped),
the platform, and the UTC generation date. After changing the API, rebuild the
client, rerun both `--out` commands above, and commit both files.

`round7b_support_table_matches_the_running_client` regenerates the reference
through the same installers and compares every capability and the Markdown
layout. It retains the committed generation metadata during comparison: another
date or build stamp alone does not change the API. The Windows snapshot is not
compared on other platforms. These files report the current capability registry;
they do not freeze the API or replace the spec's behavior tests. Forge and ImGui
parity also rely on the bridge tests and bundled UI tests.

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

### Progress Output

While waiting for the Cargo build lock, `Waiting for cargo lock` messages include the holder PID from its token file when one is present. While any step runs, a flushed progress line is printed every 60 seconds showing the step name, elapsed minutes, and—for `cargo test` steps—the number of `test ... ok/FAILED/ignored` lines seen so far; full child output continues to be written to each step's log file.

---

## 2. Bundled Mod Manifest Updater (`tools/update_bundled_manifest.py`)

Maintains exact SHA-256 checksums and file lists for bundled mods across three files:
- `bundled-mods/SOURCE.txt`
- `tools/build_player_release.py` (`BUNDLED_MOD_FILES`)
- `tools/test_player_release.py` (`BUNDLED_MOD_FILES`)

### Inclusion and Scanning Rules

Scans only files tracked by Git (`git ls-files bundled-mods`) plus untracked files under an existing mod or shared resource directory that are not git-ignored. Only runtime mod asset types matching the existing allowlist and `crates/eqoa/src/fork_ext/bundled.rs` are included:
- Extensions: `.json`, `.lua`, `.md`, `.png`, `.ttf`, `.txt`.
- Stray root files (such as `bundled-mods/SOURCE.txt` itself), dotfiles, and editor temporary/backup files (e.g. `.tmp`, `.bak`, `~`, `#*#`) are ignored.
- File bytes and line endings (`\r\n` vs `\n`) are preserved byte-identically, and any trailing text following the checksum table in `SOURCE.txt` is preserved.

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
python tools/make_owner_test_package.py --build [options]
```

Or package an already built executable:

```bash
python tools/make_owner_test_package.py --exe <path/to/release/eqoa.exe> [options]
```

### Options

- `--build`: Build `eqoa.exe` via `cargo build --locked --release -p eqoa` (using `CARGO_TARGET_DIR=%USERPROFILE%/Paseo/targets/shared`, `EQOA_BUILD_COMMIT=<HEAD>`, `EQOA_UPDATE_CHANNEL=<channel>`, `RUSTFLAGS=-C target-feature=+crt-static`, and the shared `CargoLock`) before packaging. Refuses to run when tracked files have uncommitted changes. Mutually exclusive with `--exe` (one of `--build` or `--exe` is required).
- `--exe <path>`: Path to a prebuilt release executable (mutually exclusive with `--build`).
- `--name <label>`: Package and launcher name (default: `Round5`). Used in folder name (`EQOA-Enhanced-{name}-Test`) and launcher files.
- `--channel <name>`: Release channel (default: `enhanced-dev`).
- `--output <path>`: Override package destination directory (default: `<Downloads>/EQOA-Enhanced-{name}-Test`).
- `--catalog <path>`: Optional local entity database JSON path.
- `--commit <hash>`: Optional Git commit assertion passed to `build_player_release.py`.
- `--version <ver>`: Optional version string assertion passed to `build_player_release.py`.
- `--force`: Replace `play/` and batch files in an existing output folder. Preserves existing `config/` unless `--fresh-config` is also specified.
- `--fresh-config`: Overwrite existing `config/` when `--force` is used.
- `--root <path>`: Repository root path.
