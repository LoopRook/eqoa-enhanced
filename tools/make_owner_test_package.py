#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Create a self-contained test build package for the owner."""
import argparse
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import zipfile

# Allow importing build_player_release from tools directory or repo root
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

try:
    import build_player_release
except ImportError:
    from tools import build_player_release


def format_rem_name(name):
    # If name is like 'Round4' or 'Round5', format as 'Round 4' or 'Round 5'
    if re.match(r"^[A-Za-z]+\d+$", name):
        return re.sub(r"([A-Za-z]+)(\d+)", r"\1 \2", name)
    return name


def make_package(
    *,
    name="Round5",
    exe=None,
    channel="enhanced-dev",
    output_dir=None,
    root=None,
    catalog=None,
    commit=None,
    version=None,
    force=False,
    fresh_config=False,
    config_source=None,
):
    if root is None:
        root = Path(__file__).resolve().parent.parent
    root = Path(root).resolve()

    if exe is None:
        raise ValueError("--exe is required")
    exe = Path(exe).resolve()
    if not exe.is_file():
        raise FileNotFoundError(f"Executable not found: {exe}")

    if output_dir is None:
        output_dir = Path.home() / "Downloads" / f"EQOA-Enhanced-{name}-Test"
    output_dir = Path(output_dir).resolve()

    if output_dir.exists():
        if not force:
            raise ValueError(
                f"Output directory {output_dir} already exists; use --force to overwrite"
            )
    else:
        output_dir.mkdir(parents=True, exist_ok=True)

    config_dir = output_dir / "config"
    play_dir = output_dir / "play"

    # Handle config/ directory
    if config_dir.exists() and force and not fresh_config:
        # Keep existing config
        pass
    else:
        if config_dir.exists():
            shutil.rmtree(config_dir)

        if config_source is None:
            appdata = os.environ.get("APPDATA")
            if appdata:
                config_source = Path(appdata) / "eqoa-rs"
            else:
                config_source = Path.home() / "AppData" / "Roaming" / "eqoa-rs"
        config_source = Path(config_source)

        if config_source.is_dir():
            shutil.copytree(config_source, config_dir)
        else:
            config_dir.mkdir(parents=True, exist_ok=True)

    # Handle play/ directory
    if play_dir.exists():
        shutil.rmtree(play_dir)
    play_dir.mkdir(parents=True, exist_ok=True)

    # Resolve catalog if not explicitly passed
    if catalog is None:
        default_catalog = root / "bundled-mods" / "eqoa_map_search" / "resources" / "map-entities.json"
        if default_catalog.is_file():
            catalog = default_catalog

    # Build release using build_player_release
    with tempfile.TemporaryDirectory(prefix="eqoa-pkg-stage-") as stage_temp:
        stage_path = Path(stage_temp)
        result = build_player_release.build_release(
            root=root,
            executable=exe,
            output=stage_path,
            catalog=catalog,
            commit=commit,
            version=version,
            channel=channel,
        )

        # Unpack player archive into play/
        if isinstance(result, (str, Path)) and Path(result).is_file() and zipfile.is_zipfile(result):
            with zipfile.ZipFile(result, "r") as archive:
                archive.extractall(play_dir)
        else:
            # Check stage_path for zip or files
            extracted = False
            for item in stage_path.iterdir():
                if item.is_file() and zipfile.is_zipfile(item):
                    with zipfile.ZipFile(item, "r") as archive:
                        archive.extractall(play_dir)
                    extracted = True
                    break
            if not extracted:
                for item in stage_path.iterdir():
                    dest = play_dir / item.name
                    if item.is_dir():
                        shutil.copytree(item, dest)
                    else:
                        shutil.copy2(item, dest)

    # Find the executable name inside play/
    target_exe_name = None
    expected_channel_exe = getattr(build_player_release, "exe_name", lambda ch: "")(channel)
    if expected_channel_exe and (play_dir / expected_channel_exe).is_file():
        target_exe_name = expected_channel_exe
    else:
        exe_candidates = [
            f.name for f in play_dir.glob("*.exe")
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
        "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
        f'start "" /D "%~dp0play" "%~dp0play\\{target_exe_name}"\r\n'
    )

    fps_bat_content = (
        "@echo off\r\n"
        f"rem {rem_label} test build: uses the settings copy in .\\config, never updates itself.\r\n"
        "set EQOA_CONFIG=%~dp0config\r\n"
        "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
        "set EQOA_PERF=1\r\n"
        f'start "" /D "%~dp0play" "%~dp0play\\{target_exe_name}"\r\n'
    )

    bat_path_normal = output_dir / f"Play EQOA {name} Test.bat"
    bat_path_fps = output_dir / f"Play EQOA {name} Test (fps log).bat"

    bat_path_normal.write_bytes(normal_bat_content.encode("ascii"))
    bat_path_fps.write_bytes(fps_bat_content.encode("ascii"))

    print(f"Created owner test package at: {output_dir}")
    print(f"  config: {config_dir}")
    print(f"  play: {play_dir} (executable: {target_exe_name})")
    print(f"  bats: {bat_path_normal.name}, {bat_path_fps.name}")

    return output_dir


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", default="Round5", help="package name (default: Round5)")
    parser.add_argument("--exe", type=Path, required=True, help="path to release eqoa.exe")
    parser.add_argument("--channel", default="enhanced-dev", help="release channel (default: enhanced-dev)")
    parser.add_argument("--output", type=Path, help="override output package folder")
    parser.add_argument("--root", type=Path, help="repository root")
    parser.add_argument("--catalog", type=Path, help="local catalog path for build_player_release")
    parser.add_argument("--commit", help="commit assertion for build_player_release")
    parser.add_argument("--version", help="version assertion for build_player_release")
    parser.add_argument("--force", action="store_true", help="overwrite existing output folder")
    parser.add_argument("--fresh-config", action="store_true", help="replace config even when --force is used")
    args = parser.parse_args()

    try:
        make_package(
            name=args.name,
            exe=args.exe,
            channel=args.channel,
            output_dir=args.output,
            root=args.root,
            catalog=args.catalog,
            commit=args.commit,
            version=args.version,
            force=args.force,
            fresh_config=args.fresh_config,
        )
    except (ValueError, FileNotFoundError, OSError) as error:
        parser.exit(1, f"Error: {error}\n")


if __name__ == "__main__":
    main()
