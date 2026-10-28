#!/usr/bin/env python3
"""Validate every player/updater release asset before publication."""
import argparse
import json
from pathlib import Path
from build_player_release import verify_release


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--commit")
    parser.add_argument("--version")
    args = parser.parse_args()
    manifest = verify_release(args.output)
    if args.commit and manifest["sourceCommit"] != args.commit:
        raise SystemExit("Release source does not match the tagged commit")
    if args.version and manifest["version"] != args.version:
        raise SystemExit("Release version does not match the tag")
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
