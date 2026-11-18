# Developer Tools

This document describes the Python developer helper tools located under `tools/`.

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

---

## 4. Local Loot Scene (`tools/local_scene.py`)

Run on Windows with the local test-server installation and a Frontiers disc image.

```powershell
python -B tools/local_scene.py
```

The runner uses the main checkout's `run/eqoa.iso` by default. Set `EQOA_ISO` or pass
`--iso <path>` to use another image. `--server-dir <directory>` selects a local server
installation. It must contain `eqoa-server/gameserver.exe`, its seeded `eqoa.db` and
`config.yaml`, and `accounts.local.txt` with the `lsgm` GM entry and its `Lsgm` character
in the database. `--host <IPv4>` overrides the installation's `bind_address` if the PC's
LAN address has changed. Stop any local server already listening on auth TCP 9735 or game
UDP 10070 before running the scene.

The runner snapshots the seed database for each run and suppresses the welcome dialog
for the test character in that copy. It starts the installed server binary against the
copy, selects the default addon profile in a scratch `EQOA_CONFIG`, and launches the
shared release client offscreen. If the shared release binary is missing, it builds one
with the `run_gates.py` Cargo lock. An existing shared binary is reused, including one
built from a different branch. The password is read at run time and passed to the client
through `EQOA_PASSWORD`.

The GM spawns and kills a fire bat with `#spawn fire bat` and `#kill`. `EQOA_SAY` sends
these as ordinary chat and the server recognizes the `#` prefix. The client setting
`commands to the server false` affects forwarding of unrecognized slash commands.
The game keyboard opens the corpse and takes its first item. A passing run confirms
the world entry and GM replies, the `0x0018` corpse list, the native item request and
item receipt, the server's `0x0016` close and `0x0018` refresh, and the in-world screenshot.

Each run leaves `client.log`, `server.log`, `corpse.png`, `scene.png`, and `summary.txt`
in a timestamped folder under the server installation's `runs/` directory. Failed runs
keep the evidence produced before the failure and exit with a nonzero status. The runner
stops its own server and discards the database copy on exit. This scene is run by the
lead. The parsing tests can run independently with `python -B tools/test_local_scene.py`.

### Bag Viewer scene

Build a release client with the scripted addon input changes, then run either scene.

```powershell
python -B tools/local_scene.py --scene bag-viewer
python -B tools/local_scene.py --scene bag-viewer --r65 --exe <round-6.5-release-exe>
```

The scratch profile enables the bundled Bag Viewer, binds **B** to open it, and marks
torn fire bat wings as junk. After the GM kills the bat, the develop scene uses the
game's loot key on the scripted target. It opens the Bag Viewer's **Corpse** tab and
clicks **Take all but junk**. The runner checks the client's loot receipt, reads the
Bag Viewer's own loot log through the scene diagnostic, and compares
the disposable database before and after the run to verify that meat entered the bag
and wings were left behind. `scene.png` captures the Bag Viewer's loot log.

`--r65` uses Bag Viewer's **Open nearest corpse** and requires the `takeover:` lines
for the open request, hidden loot window, and hidden session close. The round 6.5 binary
must also include the scripted addon input changes. `--exe <path>` selects a specific
build if the executable in the shared Cargo directory is from another branch.

### Addon input replay

- `EQOA_MOUSE` accepts `click X Y@ms`, `rclick @creature name@ms`, `wheel N@ms`,
  `press l|r@ms`, `release l|r@ms`, `move DX DY@ms`, `drag DX DY@ms`,
  `ctrl down|up@ms`, and `linkclick [index]@ms`. In the
  native window these use the same mouse event dispatcher, pointer claims, and ImGui
  handlers as a physical mouse. `X Y` is measured in screenshot pixels when offscreen.
- `EQOA_MOD_KEYS` accepts `b@ms`, `enter@ms`, and `text abc@ms`. A named key is
  held for 100 ms. Text types one character every 80 ms. Keys pass through the addon
  event and capture rules and reach the game when unclaimed.
- Separate steps with semicolons. `EQOA_KEYS` remains the pregame script, and
  `EQOA_PLAY_KEYS` and `EQOA_CLICKS` remain direct game scripts. `playshot` keeps
  its original `EQOA_MOUSE` path.
