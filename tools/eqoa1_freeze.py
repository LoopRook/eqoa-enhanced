#!/usr/bin/env python3
"""Extend the frozen eqoa/1 record (docs/modding/eqoa-1-frozen.json) from a generated surface file.

  python tools/eqoa1_freeze.py update [--surface docs/modding/eqoa-1-surface.json]
                                      [--record docs/modding/eqoa-1-frozen.json]
                                      [--allow-downgrade]

The record lists every `supported` path with its kind: "leaf" (no children) or "group" (has children).
`update` only ever adds paths. It refuses (exit 1, nothing written) when the surface would remove a
recorded path, lower its status below `supported`, or change its kind, unless --allow-downgrade is given;
that override needs the owner's approval and a player note (docs/modding/VERSIONING.md).
"""
import argparse, json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SURFACE = ROOT / "docs/modding/eqoa-1-surface.json"
RECORD = ROOT / "docs/modding/eqoa-1-frozen.json"


def supported_kinds(surface):
    return {
        row["path"]: ("group" if row.get("children") else "leaf")
        for row in surface["capabilities"]
        if row["status"] == "supported"
    }


def violations(frozen, surface):
    """One line per recorded path that disappeared, dropped below supported, or changed kind."""
    rows = {row["path"]: row for row in surface["capabilities"]}
    out = []
    for path, kind in frozen["supported"].items():
        row = rows.get(path)
        if row is None:
            out.append(f"{path}: supported -> removed")
        elif row["status"] != "supported":
            out.append(f"{path}: supported -> {row['status']}")
        elif ("group" if row.get("children") else "leaf") != kind:
            out.append(f"{path}: kind {kind} -> {'group' if row.get('children') else 'leaf'}")
    return out


def render(frozen):
    return json.dumps(frozen, indent=1, sort_keys=True) + "\n"


def update(surface_path, record_path, allow_downgrade):
    surface = json.loads(Path(surface_path).read_text(encoding="utf-8"))
    record_path = Path(record_path)
    old = (
        json.loads(record_path.read_text(encoding="utf-8"))
        if record_path.exists()
        else {"schema_version": 1, "api": "eqoa/1", "supported": {}}
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
    new = supported_kinds(surface)
    merged = dict(old["supported"])
    if allow_downgrade:
        for line in bad:  # the approved change: drop the entry, it re-enters if it is supported again
            merged.pop(line.split(":")[0], None)
    added = [p for p in new if p not in merged]
    merged.update({p: new[p] for p in added})
    old["supported"] = merged
    record_path.write_text(render(old), encoding="utf-8", newline="\n")
    print(f"{len(added)} added, {len(bad)} downgraded, {len(merged)} frozen -> {record_path}")
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
