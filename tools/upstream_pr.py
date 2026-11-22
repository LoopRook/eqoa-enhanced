#!/usr/bin/env python3
"""Cut and re-cut a LOCAL pull-request branch for upstream, with refusal checks.

    python tools/upstream_pr.py cut   <name> --spec <spec.json> [--worktree DIR] [--base origin/master]
    python tools/upstream_pr.py check <name> [--spec <spec.json>] [--base origin/master]
    python tools/upstream_pr.py test  <name> [--print-only]

The branch is always `upstream-pr/<name>`. The tool has no push option and refuses any other branch name.
Spec format and rules: docs/DEV-TOOLS.md, "Upstream PR tool".
"""
import argparse
import fnmatch
import json
import os
from pathlib import Path
import re
import subprocess
import sys

PREFIX = "upstream-pr/"
DEFAULT_BASE = "origin/master"

# Names that must not appear in an added line (case-insensitive). `forge_` also catches `forge_runtime`.
# `enhanced` covers Enhanced and eqoa_enhanced. A spec may allow a term with "allow_names" (a mod host PR).
FORK_NAMES = [
    "fork_",
    "forge_",
    "enhanced",
    "eqoa_enhanced",
    "looprook",
    "frontiersforge",
    "fork_ext",
    "mlua",
    r"\blua\b",
]
GAME_DATA_DIRS = ["run/", "Testing Resources/", "Testing_Resources/"]
GAME_DATA_EXTS = {
    ".iso", ".bin", ".cue", ".img", ".mdf", ".nrg", ".elf", ".cdi", ".pak", ".esf", ".snd",
    ".pem", ".key", ".p12", ".pfx", ".keystore", ".jks",
}
GAME_DATA_WORDS = ["decrypted", "credentials", "support.bin", ".env"]


class Refusal(Exception):
    pass


def git(args, cwd, check=True):
    cmd = ["git", "-c", "safe.directory=*", *args]
    r = subprocess.run(cmd, cwd=str(cwd), capture_output=True, text=True, encoding="utf-8", errors="replace")
    if check and r.returncode != 0:
        raise Refusal(f"git {' '.join(args)} failed in {cwd}: {r.stderr.strip() or r.stdout.strip()}")
    return r


def branch_name(name):
    """The only branch this tool touches. Refuses anything outside `upstream-pr/`."""
    if not name or name.startswith("-") or ".." in name or not re.fullmatch(r"[A-Za-z0-9._/-]+", name):
        raise Refusal(f"refusing branch name {name!r}: not a plain name")
    if "/" in name and not name.startswith(PREFIX):
        raise Refusal(f"refusing branch {name!r}: this tool only works on branches named {PREFIX}<name>")
    full = name if name.startswith(PREFIX) else PREFIX + name
    if full == PREFIX or full.endswith("/") or full.endswith(".lock"):
        raise Refusal(f"refusing branch name {full!r}")
    return full


def require_pr_branch(branch):
    if not branch.startswith(PREFIX) or branch == PREFIX:
        raise Refusal(f"refusing branch {branch!r}: this tool only works on branches named {PREFIX}<name>")


# ---------------------------------------------------------------- checks

def parse_diff(diff_text):
    """Yield (path, hunks); a hunk is {'start': new line, 'lines': [('+'|'-', text)]}. Needs -U0 diff."""
    files = []
    cur = None
    hunk = None
    for line in diff_text.splitlines():
        if line.startswith("diff --git "):
            cur = {"path": None, "old": None, "hunks": [], "deleted": False}
            files.append(cur)
            hunk = None
        elif cur is None:
            continue
        elif line.startswith("--- "):
            cur["old"] = None if line[4:] == "/dev/null" else line[6:]
        elif line.startswith("+++ "):
            if line[4:] == "/dev/null":
                cur["deleted"] = True
                cur["path"] = cur["old"]
            else:
                cur["path"] = line[6:]
        elif line.startswith("@@"):
            m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)", line)
            hunk = {"start": int(m.group(1)) if m else 0, "lines": []}
            cur["hunks"].append(hunk)
        elif hunk is not None and line[:1] in "+-" and not line.startswith(("+++", "---")):
            hunk["lines"].append((line[0], line[1:]))
    return [f for f in files if f["path"]]


def changed_paths(diff_text):
    return [f["path"] for f in parse_diff(diff_text)]


def _game_data_reason(path):
    low = path.lower()
    for d in GAME_DATA_DIRS:
        if low.startswith(d.lower()) or ("/" + d.lower()) in ("/" + low):
            return f"is under {d} (game data)"
    ext = os.path.splitext(low)[1]
    if ext in GAME_DATA_EXTS:
        return f"has extension {ext} (disc, game data or credential file)"
    for w in GAME_DATA_WORDS:
        if w in os.path.basename(low):
            return f"looks like game data or a credential ({w})"
    return None


def _norm(text):
    return re.sub(r"\s+", "", text)


def check_diff(diff_text, allowed_files=None, allow_names=()):
    """Return a list of error strings; empty means the diff passes."""
    errors = []
    allowed_names = {n.lower() for n in allow_names}
    patterns = [re.compile(p, re.IGNORECASE) for p in FORK_NAMES
                if p.lower().replace("\\b", "") not in allowed_names]
    for f in parse_diff(diff_text):
        path = f["path"]
        reason = _game_data_reason(path)
        if reason:
            errors.append(f"{path}: refused, {reason}")
        if allowed_files is not None and not _is_allowed(path, allowed_files):
            errors.append(f"{path}: refused, not named by the spec")
        if f["deleted"]:
            continue
        all_add = [_norm(t) for h in f["hunks"] for s, t in h["lines"] if s == "+"]
        all_del = [_norm(t) for h in f["hunks"] for s, t in h["lines"] if s == "-"]
        file_churn = bool(all_add) and sorted(all_add) == sorted(all_del)
        for h in f["hunks"]:
            adds = [t for s, t in h["lines"] if s == "+"]
            dels = [t for s, t in h["lines"] if s == "-"]
            if file_churn and h is f["hunks"][0] and len(f["hunks"]) > 1 and not (
                    adds and dels and sorted(map(_norm, adds)) == sorted(map(_norm, dels))):
                errors.append(f"{path}:{h['start']}: refused, formatting-only change "
                              f"(lines moved within the file, {len(all_add)} line(s)); upstream wants no churn")
            elif adds and dels and sorted(map(_norm, adds)) == sorted(map(_norm, dels)):
                errors.append(f"{path}:{h['start']}: refused, formatting-only change "
                              f"(whitespace or reordering, {len(adds)} line(s)); upstream wants no churn")
            elif adds and not dels and all(not t.strip() for t in adds):
                errors.append(f"{path}:{h['start']}: refused, formatting-only change (blank lines added)")
            n = h["start"]
            for s, t in h["lines"]:
                if s != "+":
                    continue
                for p in patterns:
                    m = p.search(t)
                    if m:
                        errors.append(f"{path}:{n}: refused, fork name {m.group(0)!r} in added line: {t.strip()[:100]}")
                        break
                n += 1
    return errors


def _is_allowed(path, allowed):
    return any(path == a or fnmatch.fnmatch(path, a) or (a.endswith("/") and path.startswith(a)) for a in allowed)


# ---------------------------------------------------------------- spec

def load_spec(path):
    p = Path(path)
    try:
        spec = json.loads(p.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        raise Refusal(f"cannot read spec {p}: {e}")
    if not isinstance(spec, dict):
        raise Refusal(f"spec {p}: must be a JSON object")
    known = {"message", "allowed_files", "checkout", "patches", "commits", "allow_names"}
    extra = set(spec) - known
    if extra:
        raise Refusal(f"spec {p}: unknown key(s) {sorted(extra)}")
    allowed = spec.get("allowed_files")
    if not isinstance(allowed, list) or not allowed or not all(isinstance(a, str) for a in allowed):
        raise Refusal(f"spec {p}: 'allowed_files' must be a non-empty list of paths")
    if not isinstance(spec.get("message"), str) or not spec["message"].strip():
        raise Refusal(f"spec {p}: 'message' (the commit message) is required")
    co = spec.get("checkout")
    if co is not None and not (isinstance(co, dict) and isinstance(co.get("ref"), str)
                               and isinstance(co.get("paths"), list)):
        raise Refusal(f"spec {p}: 'checkout' must be {{\"ref\": ..., \"paths\": [...]}}")
    spec["_dir"] = str(p.resolve().parent)
    return spec


# ---------------------------------------------------------------- git work

def repo_root(start):
    r = git(["rev-parse", "--show-toplevel"], start)
    return Path(r.stdout.strip()).resolve()


def default_worktree(root, name):
    return root.parent / "upstream-pr-worktrees" / name.replace("/", "_")


def _inside(child, parent):
    try:
        Path(child).resolve().relative_to(Path(parent).resolve())
        return True
    except ValueError:
        return False


def worktree_for(root, branch):
    out = git(["worktree", "list", "--porcelain"], root).stdout
    path = None
    for line in out.splitlines():
        if line.startswith("worktree "):
            path = line[9:]
        elif line == f"branch refs/heads/{branch}":
            return Path(path)
    return None


def cut(root, name, spec_path, base=DEFAULT_BASE, worktree=None):
    branch = branch_name(name)
    spec = load_spec(spec_path)
    git(["rev-parse", "--verify", base + "^{commit}"], root)
    wt = Path(worktree) if worktree else default_worktree(root, branch[len(PREFIX):])
    wt = wt.resolve()
    if wt == root or _inside(wt, root):
        raise Refusal(f"refusing worktree {wt}: it must be outside the repository, never the main checkout")
    existing = worktree_for(root, branch)
    if existing is not None and existing.resolve() != wt:
        raise Refusal(f"{branch} is already checked out at {existing}; pass --worktree {existing} to re-cut it")
    if existing is None:
        if wt.exists() and any(wt.iterdir()):
            raise Refusal(f"worktree folder {wt} exists and is not a worktree of {branch}")
        wt.parent.mkdir(parents=True, exist_ok=True)
        git(["worktree", "add", "-B", branch, str(wt), base], root)
    else:
        git(["reset", "--hard", base], wt)
        git(["clean", "-fdq"], wt)
    require_pr_branch(git(["rev-parse", "--abbrev-ref", "HEAD"], wt).stdout.strip())
    try:
        _apply(wt, spec)
        _stage_and_commit(wt, spec, base)
    except Refusal:
        git(["reset", "--hard", base], wt, check=False)
        git(["clean", "-fdq"], wt, check=False)
        raise
    return wt


def _apply(wt, spec):
    co = spec.get("checkout")
    if co and co["paths"]:
        git(["checkout", co["ref"], "--", *co["paths"]], wt)
    for patch in spec.get("patches", []):
        pp = Path(patch)
        if not pp.is_absolute():
            pp = Path(spec["_dir"]) / pp
        git(["apply", "--index", "--whitespace=nowarn", str(pp)], wt)
    for sha in spec.get("commits", []):
        git(["cherry-pick", "-n", sha], wt)


def _worktree_changes(wt):
    out = git(["status", "--porcelain", "-uall", "-z"], wt).stdout
    paths = []
    parts = out.split("\0")
    i = 0
    while i < len(parts):
        e = parts[i]
        i += 1
        if len(e) < 4:
            continue
        paths.append(e[3:])
        if e[0] in "RC":  # rename: the next field is the origin path
            paths.append(parts[i])
            i += 1
    return paths


def _stage_and_commit(wt, spec, base):
    allowed = spec["allowed_files"]
    stray = [p for p in _worktree_changes(wt) if not _is_allowed(p, allowed)]
    if stray:
        raise Refusal("\n".join(f"{p}: refused, not named by the spec" for p in stray))
    changed = _worktree_changes(wt)
    if not changed:
        raise Refusal("the spec produced no change")
    git(["add", "--", *changed], wt)  # by name, never `add -A`
    diff = git(["diff", "--cached", "-U0", base], wt).stdout
    errors = check_diff(diff, allowed, spec.get("allow_names", []))
    if errors:
        raise Refusal("\n".join(errors))
    git(["commit", "-q", "-m", spec["message"]], wt)


def check(root, name, base=DEFAULT_BASE, spec_path=None):
    branch = branch_name(name)
    spec = load_spec(spec_path) if spec_path else {}
    git(["rev-parse", "--verify", branch + "^{commit}"], root)
    diff = git(["diff", "-U0", f"{base}...{branch}"], root).stdout
    if not diff.strip():
        raise Refusal(f"{branch} has no change against {base}")
    return check_diff(diff, spec.get("allowed_files"), spec.get("allow_names", []))


def test_command():
    return ["cargo", "test", "--locked", "--workspace", "--", "--test-threads=1"]


def test(root, name, print_only=False):
    branch = branch_name(name)
    wt = worktree_for(root, branch)
    if wt is None:
        raise Refusal(f"{branch} is not checked out in any worktree; run `cut` first")
    target = os.environ.get("CARGO_TARGET_DIR") or str(Path.home() / "Paseo" / "targets" / "shared")
    if _inside(target, wt):
        raise Refusal(f"CARGO_TARGET_DIR {target} is inside the worktree; use the shared target folder")
    cmd = test_command()
    print(f"cd {wt}\nCARGO_TARGET_DIR={target}\n{' '.join(cmd)}")
    if print_only:
        return 0
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import run_gates  # the same lock every agent's build takes
    env = dict(os.environ, CARGO_TARGET_DIR=target)
    with run_gates.CargoLock():
        return subprocess.run(cmd, cwd=str(wt), env=env).returncode


# ---------------------------------------------------------------- cli

def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=".", help="a folder inside the repository (default: current directory)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("cut", help="create or reset upstream-pr/<name> from the base and apply the spec")
    c.add_argument("name")
    c.add_argument("--spec", required=True)
    c.add_argument("--worktree")
    c.add_argument("--base", default=DEFAULT_BASE)
    k = sub.add_parser("check", help="run the refusal checks on the branch's diff; changes nothing")
    k.add_argument("name")
    k.add_argument("--spec")
    k.add_argument("--base", default=DEFAULT_BASE)
    t = sub.add_parser("test", help="run upstream's tests for the branch under the shared cargo lock")
    t.add_argument("name")
    t.add_argument("--print-only", action="store_true")
    a = ap.parse_args(argv)
    try:
        root = repo_root(a.repo)
        if a.cmd == "cut":
            wt = cut(root, a.name, a.spec, a.base, a.worktree)
            print(f"cut {branch_name(a.name)} in {wt}; local only, nothing pushed")
            return 0
        if a.cmd == "check":
            errs = check(root, a.name, a.base, a.spec)
            for e in errs:
                print(e)
            print("FAIL" if errs else "PASS")
            return 1 if errs else 0
        return test(root, a.name, a.print_only)
    except Refusal as e:
        print(f"refused: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
