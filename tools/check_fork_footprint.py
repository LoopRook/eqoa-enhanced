#!/usr/bin/env python3
"""Check reviewed fork edits against upstream, including index/worktree/untracked.

Python 3.11+ and Git are required. No command changes Git state or budgets.
"""
import argparse
import fnmatch
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


def git(root, *args):
    result = subprocess.run(["git", "-C", str(root), *args], capture_output=True)
    if result.returncode:
        raise ValueError(result.stderr.decode("utf-8", errors="replace").strip())
    return result.stdout


def read_upstream(root, base, path):
    return git(root, "show", f"{base}:{path}").decode("utf-8")


def normalized(text):
    return text.replace("\r\n", "\n")


def validate_manifest(policy):
    if policy.get("schema_version") != 1:
        raise ValueError("unsupported footprint manifest schema_version")
    for path, budget in policy["shared"].items():
        if (any(char in path for char in "*?[") or not isinstance(budget, dict)
                or any(type(budget.get(key)) is not int or budget[key] < 0
                       for key in ("added", "deleted")) or not budget.get("reason")):
            raise ValueError(f"invalid shared budget: {path}")


def changed_paths(root, base):
    changes = {}
    raw = git(root, "diff", "--no-ext-diff", "--no-renames", "--numstat", "-z", base, "--")
    for record in raw.split(b"\0"):
        if not record:
            continue
        added, deleted, path = record.decode("utf-8").split("\t", 2)
        changes[path] = (None, None) if added == "-" else (int(added), int(deleted))
    for path in git(root, "ls-files", "--others", "--exclude-standard", "-z").decode("utf-8").split("\0"):
        if path:
            content = (root / path).read_bytes()
            changes[path] = (None, None) if b"\0" in content else (len(content.splitlines()), 0)
    return changes


def cargo_license(text):
    value = tomllib.loads(text)
    return tuple((section, key, table.get(key))
                 for section, table in (("package", value.get("package", {})),
                                        ("workspace.package", value.get("workspace", {}).get("package", {})))
                 for key in ("license", "license-file"))


def check(root, policy, base, show_diff=False):
    validate_manifest(policy)
    errors = []
    revision = git(root, "rev-parse", "--verify", f"{base}^{{commit}}").decode().strip()
    print(f"Fork footprint against {base} ({revision}) -> working tree + untracked")
    if policy.get("reviewed_base") and revision != policy["reviewed_base"]:
        errors.append("upstream base changed; review shared budgets and update reviewed_base explicitly")
    changed = changed_paths(root, base)
    immutable = set(policy.get("immutable", []))
    fork_count = 0
    shared_count = 0
    for path, (added, deleted) in sorted(changed.items()):
        if path in immutable:
            errors.append(f"immutable upstream file changed: {path}")
            continue
        if path == "CLAUDE.md" and policy.get("claude_upstream_copy"):
            fork_count += 1
            continue
        budget = policy["shared"].get(path)
        if budget is None and any(fnmatch.fnmatchcase(path, pattern) for pattern in policy["fork_owned"]):
            fork_count += 1
            continue
        shared_count += 1
        actual = "binary" if added is None else f"+{added} -{deleted}"
        if budget is None:
            print(f"UNREVIEWED {path}: {actual}")
            errors.append(f"unreviewed shared path: {path}")
        else:
            print(f"SHARED {path}: {actual} (budget +{budget['added']} -{budget['deleted']})")
            if added is None or added > budget["added"] or deleted > budget["deleted"]:
                errors.append(f"exceeds budget: {path}: {actual}")
        if show_diff:
            # Git diff omits untracked paths; print their additions directly.
            diff = git(root, "diff", "--no-ext-diff", "--no-renames", "--no-color", "--unified=0", base, "--", path)
            print(diff.decode("utf-8", errors="replace"), end="")
            if not diff and added is not None and (root / path).is_file():
                print(f"--- /dev/null\n+++ b/{path}")
                for line in (root / path).read_text(encoding="utf-8", errors="replace").splitlines():
                    print("+" + line)
    # CLAUDE.md is the fork's own; upstream's is kept verbatim beside it for review at each sync.
    if policy.get("claude_upstream_copy"):
        copy_path = policy["claude_upstream_copy"]
        copy = root / copy_path
        if not copy.is_file() or normalized(copy.read_text(encoding="utf-8")) != normalized(read_upstream(root, base, "CLAUDE.md")):
            errors.append(f"{copy_path} must equal upstream CLAUDE.md at {base}")
    # Inspect every tracked Cargo manifest, not only manifests with allowed edits.
    cargo_paths = {path for path in git(root, "ls-files", "-z").decode("utf-8").split("\0")
                   if path == "Cargo.toml" or path.endswith("/Cargo.toml")}
    cargo_paths.update(policy.get("license_manifests", []))
    upstream_paths = set(git(root, "ls-tree", "-r", "--name-only", "-z", base).decode("utf-8").split("\0"))
    for path in sorted(cargo_paths & upstream_paths):
        current = root / path
        if not current.is_file() or cargo_license(current.read_text(encoding="utf-8")) != cargo_license(read_upstream(root, base, path)):
            errors.append(f"Cargo license changed: {path}")
    for hook in policy.get("hooks", []):
        path = root / hook["path"]
        source = path.read_text(encoding="utf-8") if path.is_file() else ""
        count = len(re.findall(hook["pattern"], source, flags=re.MULTILINE))
        expected = hook.get("count", 1)
        print(f"HOOK {hook.get('name', hook['pattern'])}: {count}/{expected} in {hook['path']}")
        if count != expected:
            errors.append(f"hook count: {hook['path']}: {hook.get('name', hook['pattern'])}: expected {expected}, found {count}")
    print(f"Changed files: {fork_count} fork-owned, {shared_count} shared; CLAUDE/immutable checked separately.")
    for error in errors:
        print(f"ERROR: {error}")
    print("PASS: fork footprint is within reviewed policy" if not errors else f"FAIL: {len(errors)} footprint violation(s)")
    return 1 if errors else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--manifest", type=Path, default=Path("docs/fork-footprint.json"))
    parser.add_argument("--base", default="origin/master", help="Git upstream reference (default: origin/master)")
    parser.add_argument("--show-diff", action="store_true", help="also print exact +/- shared-file diff lines")
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        manifest = args.manifest if args.manifest.is_absolute() else root / args.manifest
        policy = json.loads(manifest.read_text(encoding="utf-8-sig"))
        return check(root, policy, args.base, args.show_diff)
    except (ValueError, OSError, KeyError, TypeError, re.error) as error:
        print(f"ERROR: footprint check could not complete: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
