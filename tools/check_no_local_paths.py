#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Fail closed on a Windows user-profile path recorded in a tracked file.

A hit is a drive letter, `Users`, and a user name, in either separator style.
Placeholders such as `%USERPROFILE%`, `%APPDATA%`, `$EQOA_ISO`, `<repo>` or
`<worktree:slug>` hold characters outside the user-name class, so the intended
forms never match. Binary files are skipped. A line that upstream's reviewed
base (`reviewed_base` in docs/fork-footprint.json) holds in the same file is
upstream's own text (its fixtures), not ours to rewrite, and is not a hit.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

LOCAL_PATH = re.compile(r"[A-Za-z]:[\\/]+Users[\\/]+[A-Za-z0-9._-]+")


def git(root, *args):
    result = subprocess.run(["git", "-c", "safe.directory=*", "-C", str(root), *args], capture_output=True)
    if result.returncode:
        raise ValueError(result.stderr.decode("utf-8", errors="replace").strip())
    return result.stdout


def tracked_files(root):
    return [name for name in git(root, "ls-files", "-z").decode("utf-8", errors="surrogateescape").split("\0") if name]


def upstream_lines(root, name, cache={}):
    """The stripped lines of `name` at upstream's reviewed base, or an empty set."""
    key = (str(root), name)
    if key not in cache:
        try:
            base = json.loads((root / "docs/fork-footprint.json").read_text(encoding="utf-8"))["reviewed_base"]
            text = git(root, "show", f"{base}:{name}").decode("utf-8", errors="replace")
            cache[key] = {line.strip() for line in text.splitlines()}
        except (OSError, ValueError, KeyError):
            cache[key] = set()
    return cache[key]


def scan(root):
    hits = []
    for name in tracked_files(root):
        try:
            data = (root / name).read_bytes()
        except OSError:
            continue
        if b"\0" in data[:8192]:
            continue
        for number, line in enumerate(data.decode("utf-8", errors="replace").splitlines(), 1):
            if LOCAL_PATH.search(line) and line.strip() not in upstream_lines(root, name):
                hits.append((name, number, line.strip()))
    return hits


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", nargs="?", type=Path, default=Path.cwd(), help="repository root (default: the current directory)")
    args = parser.parse_args()
    try:
        hits = scan(args.root)
    except (OSError, ValueError) as error:
        parser.exit(2, f"Local path check failed: {error}\n")
    for path, number, line in hits:
        print(f"{path}:{number}: {line}")
    if hits:
        print(f"{len(hits)} tracked lines name a local user-profile path")
        return 1
    print("No local user-profile paths in tracked files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
