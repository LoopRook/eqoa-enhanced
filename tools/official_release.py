#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Preflight and publisher of official Enhanced releases from this repository's version tags."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from build_player_release import source_commit, verify_release
from check_release_assets import check_draft, check_history, semantic_version

REPO = "LoopRook/eqoa-enhanced"
REPO_URL = f"https://github.com/{REPO}.git"


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


def validate_tag(tag, version):
    semantic_version(version)
    if tag != f"v{version}":
        raise ValueError("Immutable tag must exactly match canonical VERSION")
    return version


def check_tag(tag, expected):
    """The remote tag still names the commit that was built (lightweight or annotated)."""
    rows = command("git", "ls-remote", REPO_URL, f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}")
    refs = dict(reversed(row.split()) for row in rows.splitlines())
    actual = refs.get(f"refs/tags/{tag}^{{}}", refs.get(f"refs/tags/{tag}"))
    if actual != expected:
        raise ValueError("Release tag changed or is missing")


def preflight(root, tag):
    root = root.resolve()
    if os.environ.get("GITHUB_REPOSITORY", REPO) != REPO:
        raise ValueError(f"Official builds run only in {REPO}")
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    validate_tag(tag, version)
    commit = source_commit(root)
    if git(root, "rev-parse", f"refs/tags/{tag}^{{commit}}") != commit:
        raise ValueError("Checkout differs from the immutable version tag")
    # Players run enhanced; a version tag must be on it.
    git(root, "merge-base", "--is-ancestor", commit, "refs/remotes/origin/enhanced")
    if not (root / f"docs/releases/{version}.md").is_file():
        raise ValueError("Release notes docs/releases/<version>.md are missing")
    return {"version": version, "commit": commit}


def asset_names(version):
    return check_draft({"isDraft": True, "assets": []}, version, allow_missing=True)


def release_body(state, notes):
    return f"<!-- eqoa-build commit={state['commit']} -->\n\n{notes.strip()}\n"


def validate_draft(release, version, body):
    check_draft({"isDraft": release.get("draft"), "assets": release.get("assets")},
                version, allow_missing=True)
    if release.get("body") != body:
        raise ValueError("Draft identity/notes differ; refusing to replace another build")
    if release.get("prerelease") is not False:
        raise ValueError("Official latest release must not be a prerelease")


def validate_existing_assets(local, remote, names):
    for name in names:
        if (local / name).read_bytes() != (remote / name).read_bytes():
            raise ValueError("Existing draft asset differs; no asset will be overwritten")


def release_history():
    pages = json.loads(gh("api", "--paginate", "--slurp", f"repos/{REPO}/releases?per_page=100"))
    return [release for page in pages for release in page]


def check_remote_history(history, version):
    check_history([{"tagName": r["tag_name"], "isDraft": r["draft"]} for r in history], version)


def publish(state, output, notes):
    manifest = verify_release(output)
    if manifest["sourceCommit"] != state["commit"] or manifest["version"] != state["version"]:
        raise ValueError("Runtime assets do not match the tagged build")
    version, tag = state["version"], f"v{state['version']}"
    body = release_body(state, notes)
    check_tag(tag, state["commit"])
    history = release_history()
    check_remote_history(history, version)
    matches = [r for r in history if r["tag_name"] == tag]
    if len(matches) > 1:
        raise ValueError("Multiple releases for the same tag")
    with tempfile.TemporaryDirectory() as directory:
        temp = Path(directory)
        if matches:
            draft = matches[0]
            validate_draft(draft, version, body)
        else:
            note_file = temp / "notes.md"
            note_file.write_text(body, encoding="utf-8")
            gh("release", "create", tag, "--repo", REPO, "--verify-tag", "--draft",
               "--title", f"EQOA Enhanced {version}", "--notes-file", note_file)
            draft = {"assets": []}
        existing = [a["name"] for a in draft["assets"]]
        if existing:
            remote = temp / "existing"
            remote.mkdir()
            gh("release", "download", tag, "--repo", REPO, "--dir", remote)
            validate_existing_assets(output, remote, existing)
        for name in sorted(asset_names(version) - set(existing)):
            gh("release", "upload", tag, output / name, "--repo", REPO)
        uploaded = json.loads(gh("release", "view", tag, "--repo", REPO,
                                 "--json", "isDraft,isPrerelease,body,assets"))
        validate_draft({"draft": uploaded["isDraft"], "body": uploaded["body"],
                        "prerelease": uploaded["isPrerelease"],
                        "assets": uploaded["assets"]}, version, body)
        check_draft(uploaded, version)
        remote = temp / "complete"
        remote.mkdir()
        gh("release", "download", tag, "--repo", REPO, "--dir", remote)
        verify_release(remote)
        validate_existing_assets(output, remote, asset_names(version))
        check_tag(tag, state["commit"])
        check_remote_history(release_history(), version)
        gh("release", "edit", tag, "--repo", REPO, "--draft=false", "--prerelease=false", "--latest")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("preflight", "publish"))
    parser.add_argument("--tag", required=True)
    parser.add_argument("--assets", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        state = preflight(root, args.tag)
        if args.mode == "publish":
            if not args.assets:
                raise ValueError("--assets is required for publication")
            notes = (root / f"docs/releases/{state['version']}.md").read_text(encoding="utf-8")
            publish(state, args.assets, notes)
        elif os.environ.get("GITHUB_OUTPUT"):
            with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
                stream.write(f"commit={state['commit']}\ntag={args.tag}\n")
        print(f"Official release {args.mode} verified for {args.tag}")
    except (ValueError, OSError, KeyError) as error:
        parser.exit(1, f"Official release refused: {error}\n")


if __name__ == "__main__":
    main()
