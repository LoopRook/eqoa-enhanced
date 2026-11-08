#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Publish the rolling Enhanced development build from pushes to develop."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from build_player_release import UPDATER_ASSET, source_commit, verify_release

REPO = "LoopRook/eqoa-enhanced"
REPO_URL = f"https://github.com/{REPO}.git"
TAG = "dev"
TITLE = "EQOA Enhanced (development build)"
CHANNEL = "enhanced-dev"
BODY_PREFIX = ("Development build of develop at {commit}. May be rough; "
               "stable players should use the Latest release.")


def command(*args, cwd=None):
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, encoding="utf-8")
    if result.returncode:
        # Never echo API credentials.
        raise ValueError(f"{args[0]} operation failed (exit {result.returncode}); inspect checkout/token/release state")
    return result.stdout.strip()


def git(root, *args):
    return command("git", "-C", str(root), *args)


def gh(*args):
    return command("gh", *map(str, args))


def remote_develop():
    rows = command("git", "ls-remote", REPO_URL, "refs/heads/develop")
    refs = dict(reversed(row.split()) for row in rows.splitlines() if row.strip())
    commit = refs.get("refs/heads/develop")
    if not commit:
        raise ValueError("Remote develop branch is missing")
    return commit


def preflight(root):
    root = root.resolve()
    if os.environ.get("GITHUB_REPOSITORY", REPO) != REPO:
        raise ValueError(f"Development builds run only in {REPO}")
    commit = source_commit(root)
    # A push to develop builds that commit; a manual run may build an older one,
    # but never a commit that is not on develop.
    git(root, "merge-base", "--is-ancestor", commit, "refs/remotes/origin/develop")
    return {"commit": commit}


def asset_names(version):
    return {f"EQOA-Enhanced-{version}-Windows-x64.zip", UPDATER_ASSET,
            "update.txt", "update-notes.md", "SHA256SUMS"}


def release_body(commit, notes):
    return f"{BODY_PREFIX.format(commit=commit)}\n\n{notes.strip()}\n"


def tag_exists():
    return bool(command("git", "ls-remote", "--tags", REPO_URL, f"refs/tags/{TAG}"))


def move_tag(commit):
    """Force the rolling tag onto the built commit; create it when missing."""
    if tag_exists():
        gh("api", "-X", "PATCH", f"repos/{REPO}/git/refs/tags/{TAG}",
           "-f", f"sha={commit}", "-F", "force=true")
    else:
        gh("api", "-X", "POST", f"repos/{REPO}/git/refs",
           "-f", f"ref=refs/tags/{TAG}", "-f", f"sha={commit}")


def release_history():
    pages = json.loads(gh("api", "--paginate", "--slurp", f"repos/{REPO}/releases?per_page=100"))
    return [release for page in pages for release in page]


def find_release(history):
    matches = [release for release in history if release.get("tag_name") == TAG]
    if len(matches) > 1:
        raise ValueError("Multiple releases for the development tag")
    return matches[0] if matches else None


def validate_release(release, body, names):
    if release.get("isDraft") is not False:
        raise ValueError("Development release must be published, not a draft")
    if release.get("isPrerelease") is not True:
        raise ValueError("Development release must stay a prerelease")
    if release.get("isLatest") is not False:
        raise ValueError("Development release must never be marked latest")
    if release.get("body") != body:
        raise ValueError("Development release notes differ from the pushed build")
    assets = release.get("assets")
    if (not isinstance(assets, list)
            or any(not isinstance(asset, dict) or not isinstance(asset.get("name"), str) for asset in assets)):
        raise ValueError("Development release assets must be a list of named assets")
    actual = [asset["name"] for asset in assets]
    if len(actual) != len(set(actual)) or set(actual) != names:
        raise ValueError("Development release asset set differs from the built assets")


def publish(commit, output, notes):
    """Return "published", or "superseded" when develop has moved on already."""
    manifest = verify_release(output, channel=CHANNEL)
    if manifest["sourceCommit"] != commit:
        raise ValueError("Runtime assets do not match the checked-out commit")
    if remote_develop() != commit:
        return "superseded"
    version = manifest["version"]
    names = asset_names(version)
    body = release_body(commit, notes)
    release = find_release(release_history())
    move_tag(commit)
    with tempfile.TemporaryDirectory() as directory:
        temp = Path(directory)
        note_file = temp / "notes.md"
        note_file.write_text(body, encoding="utf-8")
        if release is None:
            gh("release", "create", TAG, "--repo", REPO, "--verify-tag",
               "--prerelease", "--latest=false", "--title", TITLE, "--notes-file", note_file)
            existing = []
        else:
            existing = [asset["name"] for asset in release.get("assets", [])]
        # The manifest goes last so no updater can see it before its payload.
        for name in sorted(names - {"update.txt"}):
            gh("release", "upload", TAG, output / name, "--repo", REPO, "--clobber")
        for name in sorted(set(existing) - names):
            gh("release", "delete-asset", TAG, name, "--repo", REPO, "--yes")
        gh("release", "upload", TAG, output / "update.txt", "--repo", REPO, "--clobber")
        gh("release", "edit", TAG, "--repo", REPO, "--prerelease", "--latest=false",
           "--title", TITLE, "--notes-file", note_file)
        validate_release(json.loads(gh("release", "view", TAG, "--repo", REPO,
                                       "--json", "isDraft,isPrerelease,isLatest,body,assets")),
                         body, names)
        remote = temp / "complete"
        remote.mkdir()
        gh("release", "download", TAG, "--repo", REPO, "--dir", remote, "--clobber")
        if verify_release(remote, channel=CHANNEL) != manifest:
            raise ValueError("Downloaded development manifest differs from the built one")
        for name in sorted(names):
            if (output / name).read_bytes() != (remote / name).read_bytes():
                raise ValueError(f"Downloaded development asset differs: {name}")
    return "published"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("preflight", "publish"))
    parser.add_argument("--assets", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        state = preflight(root)
        if args.mode == "publish":
            if not args.assets:
                raise ValueError("--assets is required for publication")
            notes = (root / "FORK-UPDATE-NOTES.md").read_text(encoding="utf-8")
            result = publish(state["commit"], args.assets, notes)
            print(f"Development release {result} for {state['commit']}")
        else:
            if os.environ.get("GITHUB_OUTPUT"):
                with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
                    stream.write(f"commit={state['commit']}\n")
            print(f"Development release preflight verified for {state['commit']}")
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"Development release refused: {error}\n")


if __name__ == "__main__":
    main()
