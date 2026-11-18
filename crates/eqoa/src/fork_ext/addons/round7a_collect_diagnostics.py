"""Collect the existing 12-package Manager diagnostic gate before/after a change.

The Rust gate runs each package, retains every Manager diagnostic, asserts all 12
ran, and fails if any diagnostic exists. Never infer empty diagnostics from an
absent, failed, ignored, or prerequisite-skipped test.
"""
import argparse
import json
from pathlib import Path

TEST = "every_bundled_mod_runs_a_play_session_with_zero_manager_diagnostics"


def round7a_collect(log, packages):
    lines = Path(log).read_text(encoding="utf-8").splitlines()
    starts = [i for i, line in enumerate(lines) if line.startswith("test ") and TEST in line]
    if len(starts) != 1:
        raise ValueError("expected exactly one bundled Manager diagnostic gate")
    start = starts[0]
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("test ")), len(lines))
    block = lines[start:end]
    passed = block[0].endswith("... ok") or "ok" in block[1:]
    if not passed or any("SKIP" in line or "FAILED" in line or "ignored" in line for line in block):
        raise ValueError("bundled diagnostic gate did not run all packages successfully")
    return {package: [] for package in packages}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[5]
    packages = sorted(p.name for p in (root / "bundled-mods").iterdir() if p.is_dir() and p.name != "resources")
    if len(packages) != 12:
        raise ValueError("bundled package set changed; review the Rust gate and collector")
    before = round7a_collect(args.before, packages)
    after = round7a_collect(args.after, packages)
    print(json.dumps({"test": TEST, "before": before, "after": after}, indent=2))


if __name__ == "__main__":
    main()
