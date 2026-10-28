#!/usr/bin/env python3
"""Fail closed on remote draft asset names and nonmonotonic semantic releases."""
import argparse
import json
from pathlib import Path
from build_player_release import UPDATER_ASSET, VERSION_PATTERN


def semantic_version(version):
    if not VERSION_PATTERN.fullmatch(version):
        raise ValueError("Version must be canonical MAJOR.MINOR.PATCH")
    return tuple(map(int, version.split(".")))


def check_draft(release, version, *, allow_missing=False):
    semantic_version(version)
    expected = {f"EQOA-Enhanced-{version}-Windows-x64.zip", UPDATER_ASSET,
                "update.txt", "update-notes.md", "SHA256SUMS"}
    if not isinstance(release, dict) or release.get("isDraft") is not True:
        raise ValueError("Only an unpublished draft may be prepared or retried")
    assets = release.get("assets")
    if (not isinstance(assets, list)
            or any(not isinstance(asset, dict) or not isinstance(asset.get("name"), str) for asset in assets)):
        raise ValueError("Remote draft assets must be a list of named assets")
    names = [asset["name"] for asset in assets]
    if len(names) != len(set(names)):
        raise ValueError("Duplicate remote draft asset names")
    extra = set(names) - expected
    if extra:
        raise ValueError(f"Unexpected remote draft assets: {sorted(extra)!r}")
    missing = expected - set(names)
    if missing and not allow_missing:
        raise ValueError(f"Missing remote draft assets: {sorted(missing)!r}")
    return expected


def check_history(releases, version):
    current = semantic_version(version)
    if not isinstance(releases, list):
        raise ValueError("Release history must be a list")
    for release in releases:
        if (not isinstance(release, dict) or not isinstance(release.get("isDraft"), bool)
                or not isinstance(release.get("tagName"), str)):
            raise ValueError("Invalid remote release history entry")
        tag = release["tagName"]
        if not release["isDraft"] and tag.startswith("v") and VERSION_PATTERN.fullmatch(tag[1:]):
            if semantic_version(tag[1:]) >= current:
                raise ValueError("Release version must exceed every published version")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("draft", "history"))
    parser.add_argument("json_file", type=Path, help="gh JSON output")
    parser.add_argument("--version", required=True)
    parser.add_argument("--allow-missing", action="store_true", help="draft pre-upload check only")
    args = parser.parse_args()
    try:
        payload = json.loads(args.json_file.read_text(encoding="utf-8"))
        if args.mode == "draft":
            check_draft(payload, args.version, allow_missing=args.allow_missing)
        else:
            if args.allow_missing:
                raise ValueError("--allow-missing applies only to draft pre-upload checks")
            check_history(payload, args.version)
    except (ValueError, OSError) as error:
        parser.exit(1, f"Release guard failed: {error}\n")
    print(f"Release {args.mode} verified for {args.version}")


if __name__ == "__main__":
    main()
