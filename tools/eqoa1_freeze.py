#!/usr/bin/env python3
"""Extend the frozen eqoa/1 record (docs/modding/eqoa-1-frozen.json) from a generated surface file.

  python tools/eqoa1_freeze.py update [--surface docs/modding/eqoa-1-surface.json]
                                      [--record docs/modding/eqoa-1-frozen.json]
                                      [--allow-downgrade]

The record lists every `supported` path (names only). `update` only ever adds paths and does not touch
the file when nothing is added. It refuses (exit 1, nothing written) when the surface would remove a
recorded path or lower its status below `supported`, unless --allow-downgrade is given;
that override needs the owner's approval and a player note (docs/modding/VERSIONING.md).
"""
import argparse, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SURFACE = ROOT / "docs/modding/eqoa-1-surface.json"
RECORD = ROOT / "docs/modding/eqoa-1-frozen.json"


def supported_paths(surface):
    return {row["path"] for row in surface["capabilities"] if row["status"] == "supported"}


def violations(frozen, surface):
    """One line per recorded path that disappeared or dropped below supported."""
    rows = {row["path"]: row for row in surface["capabilities"]}
    out = []
    for path in frozen["supported"]:
        row = rows.get(path)
        if row is None:
            out.append(f"{path}: supported -> removed")
        elif row["status"] != "supported":
            out.append(f"{path}: supported -> {row['status']}")
    return out


def render(frozen):
    return json.dumps(frozen, indent=1, sort_keys=True) + "\n"


def update(surface_path, record_path, allow_downgrade):
    surface = json.loads(Path(surface_path).read_text(encoding="utf-8"))
    record_path = Path(record_path)
    old = (
        json.loads(record_path.read_text(encoding="utf-8"))
        if record_path.exists()
        else {"schema_version": 1, "api": "eqoa/1", "supported": []}
    )
    bad = violations(old, surface)
    if bad and not allow_downgrade:
        print("refusing to write: the surface removes or lowers frozen eqoa/1 names:", file=sys.stderr)
        for line in bad:
            print("  " + line, file=sys.stderr)
        print("A downgrade or removal under eqoa/1 needs the owner's approval and a player note;"
              " a breaking change is eqoa/2. Re-run with --allow-downgrade only with that approval.",
              file=sys.stderr)
        return 1
    merged = set(old["supported"])
    if allow_downgrade:
        merged -= {line.split(":")[0] for line in bad}  # the approved change
    added = supported_paths(surface) - merged
    merged |= added
    old["supported"] = sorted(merged)
    if added or bad or not record_path.exists():
        record_path.write_text(render(old), encoding="utf-8", newline="\n")
        print(f"{len(added)} added, {len(bad)} downgraded, {len(merged)} frozen -> {record_path.name}")
    else:
        print(f"nothing to add; {record_path.name} left untouched")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["update"])
    ap.add_argument("--surface", default=str(SURFACE))
    ap.add_argument("--record", default=str(RECORD))
    ap.add_argument("--allow-downgrade", action="store_true")
    a = ap.parse_args(argv)
    return update(a.surface, a.record, a.allow_downgrade)


if __name__ == "__main__":
    sys.exit(main())
