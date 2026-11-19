#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Create a self-contained test build package for the owner."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

# Allow importing build_player_release and run_gates from tools directory or repo root
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

try:
    import build_player_release
    import run_gates
except ImportError:
    from tools import build_player_release
    from tools import run_gates


ARCHIVE_SCRIPT_NAME = "archive_previous_run.ps1"

# Sets the previous run's logs aside in results\<date-time>\ (moved, so a third launch
# cannot erase the first). Moves logs, the state snapshot and local recordings.
ARCHIVE_SCRIPT = r"""# Sets the previous run's logs aside so no run is lost. Called by the Play .bat files.
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$dest = Join-Path $root (Join-Path 'results' $stamp)
$n = 1
while (Test-Path $dest) { $n++; $dest = Join-Path $root (Join-Path 'results' ($stamp + '-' + $n)) }
foreach ($top in 'config', 'play') {
    $dir = Join-Path $root $top
    if (-not (Test-Path $dir)) { continue }
    Get-ChildItem -Path $dir -Recurse -File | Where-Object {
        $_.Name -like 'eqoa*.log' -or $_.Name -like 'addons_error.log*' -or $_.Name -eq 'state.txt' -or
            ($top -eq 'config' -and $_.DirectoryName -eq $dir -and $_.Name -like 'session-*.eqrec')
    } | ForEach-Object {
        $rel = $_.FullName.Substring($root.Length).TrimStart('\', '/')
        $target = Join-Path $dest $rel
        New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
        Move-Item -LiteralPath $_.FullName -Destination $target
    }
}
"""

ARCHIVE_CALL = (
    "powershell -NoProfile -ExecutionPolicy Bypass -File "
    f'"%~dp0{ARCHIVE_SCRIPT_NAME}"\r\n'
)


class LiveConfigOverlapError(ValueError):
    """Raised when output folder overlaps the live configuration directory."""
    pass


def resolve_live_config():
    appdata = os.environ.get("APPDATA")
    if appdata:
        return (Path(appdata) / "eqoa-rs").resolve()
    return (Path.home() / "AppData" / "Roaming" / "eqoa-rs").resolve()


def is_symlink_or_junction(path):
    p = Path(path)
    if p.is_symlink():
        return True
    try:
        import stat
        s = p.lstat()
        if hasattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT"):
            if s.st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT:
                return True
    except (OSError, AttributeError):
        pass
    return False


def format_rem_name(name):
    if re.match(r"^[A-Za-z]+\d+$", name):
        return re.sub(r"([A-Za-z]+)(\d+)", r"\1 \2", name)
    return name


def _build_release_exe(root, channel):
    """Build eqoa.exe with commit, channel, and static-CRT env under CargoLock."""
    status = subprocess.run(
        [
            "git",
            "-c",
            "safe.directory=*",
            "-C",
            str(root),
            "status",
            "--porcelain",
            "--untracked-files=no",
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if status.returncode != 0:
        raise RuntimeError(f"git status failed: {status.stderr.strip()}")
    if status.stdout.strip():
        raise ValueError("Refusing --build: worktree has uncommitted changes to tracked files")

    rev = subprocess.run(
        ["git", "-c", "safe.directory=*", "-C", str(root), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if rev.returncode != 0 or not rev.stdout.strip():
        raise RuntimeError(f"git rev-parse HEAD failed: {rev.stderr.strip()}")
    head_commit = rev.stdout.strip()

    cargo_target_dir = os.environ.get("CARGO_TARGET_DIR")
    if not cargo_target_dir:
        cargo_target_dir = str(Path.home() / "Paseo" / "targets" / "shared")
    cargo_target_dir = Path(cargo_target_dir)

    build_env = os.environ.copy()
    build_env["CARGO_TARGET_DIR"] = str(cargo_target_dir)
    build_env["EQOA_BUILD_COMMIT"] = head_commit
    build_env["EQOA_UPDATE_CHANNEL"] = channel
    rustflags = build_env.get("RUSTFLAGS", "")
    if "-C target-feature=+crt-static" not in rustflags:
        build_env["RUSTFLAGS"] = f"{rustflags} -C target-feature=+crt-static".strip()

    cmd = ["cargo", "build", "--locked", "--release", "-p", "eqoa"]
    exe_path = run_gates._release_exe_path(cargo_target_dir).resolve()
    with run_gates.CargoLock():
        # Always touch: the shared target dir may hold another worktree's exe, and the stamped
        # commit must come from a build of this tree (the touch also reruns build.rs).
        guard = run_gates.SharedTargetGuard(root, cargo_target_dir)
        guard.begin(force=True)
        proc = subprocess.run(
            cmd,
            cwd=root,
            env=build_env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        stale = None
        if proc.returncode == 0:
            try:
                (_, mtime), = guard.verify([exe_path])
                print(f"Built {run_gates.describe_exe(exe_path, mtime)}")
            except run_gates.StaleBinaryError as exc:
                stale = exc
        guard.end()

    combined = "\n".join(part for part in (proc.stdout, proc.stderr) if part)
    summary_lines = [
        line.strip()
        for line in combined.splitlines()
        if line.lstrip().startswith(("Finished ", "error", "could not compile"))
    ]
    for line in summary_lines:
        print(line)
    if proc.returncode != 0:
        if not summary_lines:
            tail = [line.strip() for line in combined.splitlines() if line.strip()][-10:]
            for line in tail:
                print(line)
        raise RuntimeError(f"cargo build failed with exit code {proc.returncode}")
    if stale is not None:
        raise RuntimeError(f"stale binary: {stale}")

    return exe_path


def make_package(
    *,
    name="Round5",
    exe=None,
    build=False,
    channel="enhanced-dev",
    output_dir=None,
    root=None,
    catalog=None,
    commit=None,
    version=None,
    force=False,
    fresh_config=False,
    config_source=None,
    checklist=None,
):
    if root is None:
        root = Path(__file__).resolve().parent.parent
    root = Path(root).resolve()

    if build and exe is not None:
        raise ValueError("Cannot specify both --exe and --build")
    if not build:
        if exe is None:
            raise ValueError("--exe is required")
        exe = Path(exe).resolve()
        if not exe.is_file():
            raise FileNotFoundError(f"Executable not found: {exe}")

    if checklist is not None:
        checklist = Path(checklist).resolve()
        if not checklist.is_file():
            raise FileNotFoundError(f"Checklist not found: {checklist.name}")

    if output_dir is None:
        output_dir = Path.home() / "Downloads" / f"EQOA-Enhanced-{name}-Test"
    output_dir = Path(output_dir).resolve()

    # Safety check: live config overlap
    live_config = resolve_live_config()
    try:
        if (
            output_dir == live_config
            or output_dir.is_relative_to(live_config)
            or live_config.is_relative_to(output_dir)
        ):
            raise LiveConfigOverlapError(
                f"Output directory {output_dir} equals, contains, or is inside live config directory {live_config}"
            )
    except AttributeError:
        # Fallback for Python versions without is_relative_to (Python 3.9+)
        out_str = str(output_dir).rstrip("\\/")
        live_str = str(live_config).rstrip("\\/")
        if (
            out_str == live_str
            or out_str.startswith(live_str + os.sep)
            or live_str.startswith(out_str + os.sep)
        ):
            raise LiveConfigOverlapError(
                f"Output directory {output_dir} equals, contains, or is inside live config directory {live_config}"
            )

    config_dir = output_dir / "config"
    play_dir = output_dir / "play"

    # Safety check: junction/symlink on config
    if config_dir.exists() and is_symlink_or_junction(config_dir):
        raise LiveConfigOverlapError(
            f"Config directory {config_dir} is a symlink or junction; refusing to write"
        )

    if output_dir.exists() and not force:
        raise ValueError(
            f"Output directory {output_dir} already exists; use --force to overwrite"
        )

    if build:
        exe = _build_release_exe(root, channel)
        if not exe.is_file():
            raise FileNotFoundError(f"Executable not found: {exe}")

    # Resolve catalog if not explicitly passed
    if catalog is None:
        default_catalog = root / "bundled-mods" / "eqoa_map_search" / "resources" / "map-entities.json"
        if default_catalog.is_file():
            catalog = default_catalog

    # Build and stage into a temporary staging folder next to output_dir
    output_dir.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".eqoa-owner-stage-", dir=output_dir.parent) as stage_temp:
        stage_dir = Path(stage_temp)
        stage_build = stage_dir / "build_output"
        stage_play = stage_dir / "play"
        stage_play.mkdir(parents=True, exist_ok=True)

        result = build_player_release.build_release(
            root=root,
            executable=exe,
            output=stage_build,
            catalog=catalog,
            commit=commit,
            version=version,
            channel=channel,
        )

        # Unpack player archive into stage_play
        if isinstance(result, (str, Path)) and Path(result).is_file() and zipfile.is_zipfile(result):
            with zipfile.ZipFile(result, "r") as archive:
                archive.extractall(stage_play)
        else:
            extracted = False
            if stage_build.is_dir():
                for item in stage_build.iterdir():
                    if item.is_file() and zipfile.is_zipfile(item):
                        with zipfile.ZipFile(item, "r") as archive:
                            archive.extractall(stage_play)
                        extracted = True
                        break
                if not extracted:
                    for item in stage_build.iterdir():
                        dest = stage_play / item.name
                        if item.is_dir():
                            shutil.copytree(item, dest)
                        else:
                            shutil.copy2(item, dest)

        # Find target_exe_name in stage_play
        target_exe_name = None
        expected_channel_exe = getattr(build_player_release, "exe_name", lambda ch: "")(channel)
        if expected_channel_exe and (stage_play / expected_channel_exe).is_file():
            target_exe_name = expected_channel_exe
        else:
            exe_candidates = [
                f.name for f in stage_play.glob("*.exe")
                if f.name != "eqoa-enhanced-windows-x86_64.exe"
            ]
            if exe_candidates:
                target_exe_name = exe_candidates[0]
            else:
                target_exe_name = expected_channel_exe or "EQOA Enhanced - Nightly.exe"

        rem_label = format_rem_name(name)
        normal_bat_content = (
            "@echo off\r\n"
            f"rem {rem_label} test build: uses the settings copy in .\\config, never updates itself.\r\n"
            "set EQOA_CONFIG=%~dp0config\r\n"
            "set EQOA_RECORD_SESSION=1\r\n"
            "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
            + ARCHIVE_CALL
            + f'start "" /D "%~dp0play" "%~dp0play\\{target_exe_name}"\r\n'
        )

        fps_bat_content = (
            "@echo off\r\n"
            f"rem {rem_label} test build: uses the settings copy in .\\config, never updates itself.\r\n"
            "set EQOA_CONFIG=%~dp0config\r\n"
            "set EQOA_RECORD_SESSION=1\r\n"
            "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
            "set EQOA_PERF=1\r\n"
            + ARCHIVE_CALL
            + f'start "" /D "%~dp0play" "%~dp0play\\{target_exe_name}"\r\n'
        )

        stage_bat_normal = stage_dir / f"Play EQOA {name} Test.bat"
        stage_bat_fps = stage_dir / f"Play EQOA {name} Test (fps log).bat"
        stage_bat_normal.write_bytes(normal_bat_content.encode("ascii"))
        stage_bat_fps.write_bytes(fps_bat_content.encode("ascii"))

        (stage_dir / ARCHIVE_SCRIPT_NAME).write_bytes(
            ARCHIVE_SCRIPT.replace("\n", "\r\n").encode("ascii")
        )

        # Build succeeded! Now apply to destination output_dir
        output_dir.mkdir(parents=True, exist_ok=True)

        # Handle config/ directory
        if config_dir.exists() and force and not fresh_config:
            # Keep existing config
            pass
        else:
            if config_dir.exists():
                shutil.rmtree(config_dir)

            if config_source is None:
                config_source = live_config
            config_source = Path(config_source)

            if config_source.is_dir():
                shutil.copytree(
                    config_source, config_dir,
                    ignore=lambda _folder, names: [name for name in names if name.lower().endswith(".eqrec")],
                )
            else:
                config_dir.mkdir(parents=True, exist_ok=True)

        # Replace play/
        if play_dir.exists():
            shutil.rmtree(play_dir)
        shutil.copytree(stage_play, play_dir)

        # Replace bats
        shutil.copy2(stage_bat_normal, output_dir / f"Play EQOA {name} Test.bat")
        shutil.copy2(stage_bat_fps, output_dir / f"Play EQOA {name} Test (fps log).bat")
        shutil.copy2(stage_dir / ARCHIVE_SCRIPT_NAME, output_dir / ARCHIVE_SCRIPT_NAME)
        if checklist is not None:
            shutil.copy2(checklist, output_dir / "CHECKLIST.txt")

    print(f"Created owner test package at: {output_dir}")
    print(f"  config: {config_dir}")
    print(f"  play: {play_dir} (executable: {target_exe_name})")
    print(f"  bats: Play EQOA {name} Test.bat, Play EQOA {name} Test (fps log).bat")

    return output_dir


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", default="Round5", help="package name (default: Round5)")
    source_group = parser.add_mutually_exclusive_group(required=True)
    source_group.add_argument("--exe", type=Path, help="path to release eqoa.exe")
    source_group.add_argument(
        "--build",
        action="store_true",
        help="build release eqoa.exe with commit/channel/crt-static env and shared CargoLock",
    )
    parser.add_argument("--channel", default="enhanced-dev", help="release channel (default: enhanced-dev)")
    parser.add_argument("--output", type=Path, help="override output package folder")
    parser.add_argument("--root", type=Path, help="repository root")
    parser.add_argument("--catalog", type=Path, help="local catalog path for build_player_release")
    parser.add_argument("--commit", help="commit assertion for build_player_release")
    parser.add_argument("--version", help="version assertion for build_player_release")
    parser.add_argument("--force", action="store_true", help="overwrite existing output folder")
    parser.add_argument("--fresh-config", action="store_true", help="replace config even when --force is used")
    parser.add_argument("--checklist", type=Path, help="copy this round's checklist into the package as CHECKLIST.txt")
    args = parser.parse_args()

    try:
        make_package(
            name=args.name,
            exe=args.exe,
            build=args.build,
            channel=args.channel,
            output_dir=args.output,
            root=args.root,
            catalog=args.catalog,
            commit=args.commit,
            version=args.version,
            force=args.force,
            fresh_config=args.fresh_config,
            checklist=args.checklist,
        )
    except LiveConfigOverlapError as error:
        parser.exit(2, f"Security error: {error}\n")
    except (ValueError, FileNotFoundError, OSError, RuntimeError) as error:
        parser.exit(1, f"Error: {error}\n")


if __name__ == "__main__":
    main()
