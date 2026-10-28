"""Launch the parser manifest without a shell.

EQOA_PARSER_DIR names the source/installation working directory. Relative values
are resolved against the client installation directory. Relative manifest commands
are resolved beside mod.json; bare commands prefer that directory, then PATH.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

class ParserLaunchError(RuntimeError):
    """An actionable companion configuration or process creation failure."""


def launch_parser(base_dir):
    base = Path(base_dir).resolve()
    manifest_path = base / "mods" / "eqoa_parser" / "mod.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise ParserLaunchError(f"Cannot read parser manifest {manifest_path}: {exc}") from exc
    if not isinstance(manifest, dict) or manifest.get("id") != "eqoa_parser":
        raise ParserLaunchError("Parser manifest id must be eqoa_parser.")
    if manifest.get("enabled", True) is not True:
        raise ParserLaunchError("The parser is disabled in mod.json.")
    command = manifest.get("command")
    args = manifest.get("args", [])
    if not isinstance(command, str) or not command.strip():
        raise ParserLaunchError("Parser manifest command must be a nonempty string.")
    if not isinstance(args, list) or any(not isinstance(arg, str) for arg in args):
        raise ParserLaunchError("Parser manifest args must be an array of strings.")
    configured = os.environ.get("EQOA_PARSER_DIR", "")
    if not configured.strip():
        raise ParserLaunchError("Set EQOA_PARSER_DIR to the parser source or installation directory.")
    working_dir = Path(configured)
    if not working_dir.is_absolute():
        working_dir = base / working_dir
    if not working_dir.is_dir():
        raise ParserLaunchError(f"EQOA_PARSER_DIR is not a directory: {working_dir}")
    working_dir = working_dir.resolve()
    program = Path(command)
    if program.is_absolute():
        resolved = program if program.is_file() else None
    elif "/" in command or "\\" in command:
        program = manifest_path.parent / program
        resolved = program if program.is_file() else None
    else:
        resolved = shutil.which(command, path=str(manifest_path.parent)) or shutil.which(command)
    if not resolved:
        raise ParserLaunchError(f"Parser command was not found beside mod.json or on PATH: {command}")
    flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
    try:
        return subprocess.Popen([str(resolved), *args], cwd=working_dir, creationflags=flags)
    except OSError as exc:
        raise ParserLaunchError(f"Cannot launch parser command {command}: {exc}") from exc


def main():
    try:
        launch_parser(Path(__file__).resolve().parent.parent)
    except ParserLaunchError as exc:
        print(f"EQOA parser: {exc}", file=sys.stderr)
        return 1
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
