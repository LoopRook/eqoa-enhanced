#!/usr/bin/env python3
# LoopRook's additions in this file are under the GNU Affero General Public License v3 or later (see LICENSE).
"""Merge upstream (origin/master) into a fresh sync/<date> branch and do the routine parts.

    python tools/sync_upstream.py                 # fetch, worktree, merge, resolve, check, test
    python tools/sync_upstream.py --continue      # in the sync worktree, after hand-resolving

Routine parts: README.md and CLAUDE.md keep ours (.gitattributes), Git's rerere replays
resolutions recorded in earlier syncs, and a conflict inside one-line struct literals
(play.rs's `Play { ... }`) is merged field by field. Anything else stops with a list of
conflicts for a person to resolve. A clean merge then refreshes docs/upstream/CLAUDE.md,
moves reviewed_base only if every shared budget still passes, commits the merge, and runs
the client/net tests and the Python tool tests. Nothing is pushed or merged into develop.

Exit codes: 0 done (or nothing to sync), 1 tests failed, 2 conflicts need a person,
3 footprint budgets need review, 4 setup error, 5 merged but tests not run (another build).
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import subprocess
import sys

UPSTREAM = "origin/master"
MANIFEST = "docs/fork-footprint.json"
CLAUDE_COPY = "docs/upstream/CLAUDE.md"
REPORT = "target/sync-report.md"
PYTHON = sys.executable


class SyncError(Exception):
    pass


def git(root, *args, check=True):
    result = subprocess.run(["git", "-c", "safe.directory=*", "-C", str(root), *args],
                            capture_output=True, text=True, encoding="utf-8", errors="replace")
    if check and result.returncode:
        raise SyncError(f"git {' '.join(args)}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout


# ---------------------------------------------------------------------------
# One-line struct literal merge


def split_top_level(body):
    """Split `a, b: f(x, y), c` on commas outside brackets and string/char literals."""
    parts, depth, start, i = [], 0, 0, 0
    while i < len(body):
        c = body[i]
        if c == '"':
            i += 1
            while i < len(body) and body[i] != '"':
                i += 2 if body[i] == "\\" else 1
        elif c == "'" and i + 2 < len(body) and (body[i + 2] == "'" or body[i + 1] == "\\"):
            i = body.index("'", i + 3 if body[i + 1] == "\\" else i + 2)
        elif c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth < 0:
                return None
        elif c == "," and depth == 0:
            parts.append(body[start:i].strip())
            start = i + 1
        i += 1
    if depth:
        return None
    last = body[start:].strip()
    if last:
        parts.append(last)
    return parts


ATTR = re.compile(r"\s*#\[")


def field_key(field):
    rest = field
    while ATTR.match(rest):
        depth, i = 0, rest.index("#[") + 1
        for i in range(i, len(rest)):
            depth += {"[": 1, "]": -1}.get(rest[i], 0)
            if depth == 0:
                break
        rest = rest[i + 1:]
    rest = rest.strip()
    if rest.startswith(".."):
        return ".."
    m = re.match(r"(r#)?[A-Za-z_]\w*", rest)
    if not m or rest[m.end():].strip()[:1] not in ("", ":"):
        return None
    return m.group(0)


LITERAL = re.compile(r"^(?P<head>.*?\b[A-Z]\w*\s*\{\s*)(?P<body>.*?)(?P<tail>\s*\}\s*;?\s*)$", re.S)


# Raw strings (r, br, cr) hold unescaped quotes and commas; comments can hold anything.
UNSUPPORTED = re.compile(r'(?<![A-Za-z0-9_])[bc]?r#*"|//|/\*')


def parse_literal(line):
    m = LITERAL.match(line)
    if not m or UNSUPPORTED.search(m.group("body")):
        return None
    fields = split_top_level(m.group("body"))
    # The merge rejoins fields with ", ": refuse a body that would not come back byte for byte.
    if not fields or len(fields) < 2 or ", ".join(fields) != m.group("body").strip():
        return None
    keyed = []
    for f in fields:
        k = field_key(f)
        if k is None or k in (key for key, _ in keyed):
            return None
        keyed.append((k, f))
    return m.group("head"), keyed, m.group("tail")


def merge_struct_literal(base, ours, theirs):
    """Three-way merge of one-line struct literals; None when it is not that or conflicts.

    Theirs' order wins; fields only ours added go after their predecessor in ours.
    A field changed on one side takes that side; changed differently on both is a conflict,
    as is ours changing a field upstream removed or upstream changing one ours removed.
    """
    parsed = [parse_literal(x) for x in (base, ours, theirs)]
    if None in parsed:
        return None
    (hb, b, tb), (ho, o, to), (ht, t, tt) = parsed
    if not (hb == ho == ht or (ho == hb and ht) or (ht == hb and ho)):
        return None
    head = ht if ho == hb else ho
    tail = tt if to == tb else to
    if ho != hb and ht != hb and ho != ht:
        return None
    if to != tb and tt != tb and to != tt:
        return None
    B, O, T = dict(b), dict(o), dict(t)
    # Upstream's order wins below, which would silently undo a reordering (evaluation
    # order) on our side: refuse when ours reordered fields it shares with the base.
    if [k for k, _ in o if k in B] != [k for k, _ in b if k in O]:
        return None
    result = []
    for k, tv in t:
        if k in B and k not in O:
            if tv != B[k]:
                return None
            continue
        if k in O:
            ov = O[k]
            bv = B.get(k)
            if ov == tv or ov == bv:
                result.append((k, tv))
            elif tv == bv:
                result.append((k, ov))
            else:
                return None
        else:
            result.append((k, tv))
    for k, bv in b:
        if k not in T and k in O and O[k] != bv:
            return None
    keys = [k for k, _ in result]
    for idx, (k, ov) in enumerate(o):
        if k in B or k in T:
            continue
        at = 0
        for prev, _ in reversed(o[:idx]):
            if prev in keys:
                at = keys.index(prev) + 1
                break
        result.insert(at, (k, ov))
        keys.insert(at, k)
    if ".." in keys and keys[-1] != "..":
        return None
    return head + ", ".join(v for _, v in result) + tail


# ---------------------------------------------------------------------------
# Conflict hunks (diff3 style)

HUNK = re.compile(
    r"^<<<<<<< [^\n]*\n(?P<ours>.*?)^\|\|\|\|\|\|\| [^\n]*\n(?P<base>.*?)^=======\r?\n(?P<theirs>.*?)^>>>>>>> [^\n]*\n",
    re.S | re.M)


def single_line(text):
    lines = text.splitlines(keepends=True)
    return lines[0] if len(lines) == 1 else None


def resolve_text(text):
    """Resolve the struct-literal hunks in a diff3-conflicted file.

    Returns (new_text, resolved_count, remaining_count).
    """
    resolved, remaining, out, pos = 0, 0, [], 0
    for m in HUNK.finditer(text):
        out.append(text[pos:m.start()])
        pos = m.end()
        o, b, t = (single_line(m.group(g)) for g in ("ours", "base", "theirs"))
        merged = None
        if o and b and t:
            ending = o[len(o.rstrip("\r\n")):]
            merged = merge_struct_literal(b.rstrip("\r\n"), o.rstrip("\r\n"), t.rstrip("\r\n"))
        if merged is None:
            out.append(m.group(0))
            remaining += 1
        else:
            out.append(merged + ending)
            resolved += 1
    out.append(text[pos:])
    new = "".join(out)
    if remaining == 0 and "<<<<<<< " in new:
        remaining = new.count("<<<<<<< ")
    return new, resolved, remaining


def has_markers(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    return bool(re.search(r"^(<<<<<<<|>>>>>>>) ", text, re.M))


# ---------------------------------------------------------------------------
# Steps


def unmerged(root):
    return sorted(set(line.split("\t", 1)[1] for line in git(root, "ls-files", "-u").splitlines() if "\t" in line))


REPLAYED = re.compile(r"^Resolved '(.+)' using previous resolution\.$", re.M)


def stages(root, rel):
    return {line.split()[2] for line in git(root, "ls-files", "-u", "--", rel).splitlines()}


def auto_resolve(root, notes, replayed=()):
    """Stage files rerere reported resolving, merge struct literals; return the rest.

    A conflicted file without markers is staged only when rerere said it replayed it:
    binary and modify/delete conflicts carry no markers either, and are a person's call.
    """
    left = []
    for rel in unmerged(root):
        path = root / rel
        if stages(root, rel) != {"1", "2", "3"} or not path.is_file():
            left.append(f"{rel} (added or deleted on one side)")
            continue
        if not has_markers(path):
            if rel in replayed:
                git(root, "add", "--", rel)
                notes.append(f"{rel}: replayed an earlier sync's resolution (rerere); read its diff")
            else:
                left.append(f"{rel} (binary, or no conflict markers)")
            continue
        raw = path.read_bytes().decode("utf-8")
        new, done, remaining = resolve_text(raw)
        if done:
            path.write_bytes(new.encode("utf-8"))
            notes.append(f"{rel}: {done} one-line struct literal(s) merged field by field")
        if remaining:
            left.append(f"{rel} ({remaining} hunk(s))")
        else:
            git(root, "add", "--", rel)
    return left


def refresh_claude_copy(root, upstream):
    old = (root / CLAUDE_COPY).read_text(encoding="utf-8").replace("\r\n", "\n")
    new = git(root, "show", f"{upstream}:CLAUDE.md")
    (root / CLAUDE_COPY).write_text(new, encoding="utf-8", newline="\n")
    git(root, "add", "--", CLAUDE_COPY)
    return old != new.replace("\r\n", "\n")


def move_reviewed_base(root, sha):
    """Point reviewed_base at sha; returns (old base, original bytes to restore on failure)."""
    path = root / MANIFEST
    original = path.read_bytes()
    raw = original.decode("utf-8")
    old = json.loads(raw)["reviewed_base"]
    if old != sha:
        path.write_bytes(raw.replace(f'"reviewed_base": "{old}"', f'"reviewed_base": "{sha}"', 1).encode("utf-8"))
    return old, original


def footprint(root, sha):
    r = subprocess.run([PYTHON, "tools/check_fork_footprint.py", "--base", sha],
                       cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace")
    return r.returncode == 0, r.stdout + r.stderr


def other_cargo_running():
    if sys.platform != "win32":
        return False
    r = subprocess.run(["tasklist", "/FI", "IMAGENAME eq cargo.exe", "/NH"], capture_output=True, text=True)
    return "cargo.exe" in r.stdout


def run_tests(root):
    target = root / "target"
    config = target / "sync-config"
    config.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=os.environ.get("CARGO_TARGET_DIR", str(target)), EQOA_CONFIG=str(config))
    cargo = subprocess.run(["cargo", "test", "--locked", "-p", "eqoa", "-p", "net", "--", "--nocapture", "--test-threads=1"],
                           cwd=root, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    log = cargo.stdout + cargo.stderr
    (target / "sync-tests.log").write_text(log, encoding="utf-8")
    passed = failed = ignored = 0
    for m in re.finditer(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", log):
        passed, failed, ignored = passed + int(m[1]), failed + int(m[2]), ignored + int(m[3])
    skips = len(re.findall(r"\bSKIP\b", log))
    py = subprocess.run([PYTHON, "-X", "utf8", "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py"],
                        cwd=root, capture_output=True, text=True, encoding="utf-8", errors="replace")
    py_line = (re.findall(r"Ran \d+ tests?.*", py.stderr) or ["(no summary)"])[-1]
    ok = cargo.returncode == 0 and py.returncode == 0
    summary = (f"cargo test -p eqoa -p net: {'PASS' if cargo.returncode == 0 else 'FAIL'}, "
               f"{passed} passed / {failed} failed / {ignored} ignored, {skips} SKIP messages "
               f"(missing disc/oracle prerequisites); log target/sync-tests.log\n"
               f"Python tools: {'PASS' if py.returncode == 0 else 'FAIL'}, {py_line.strip()}")
    if py.returncode:
        summary += "\n" + py.stderr[-3000:]
    return ok, summary


def test_step(root):
    """(exit code, summary); 5 when another cargo build is running, as one runs at a time."""
    if other_cargo_running():
        return 5, ("Not run: another cargo build is running (one build at a time). Rerun "
                   "`python tools/sync_upstream.py --continue --tests-only` in this worktree when it finishes.")
    ok, summary = run_tests(root)
    return (0 if ok else 1), summary


def write_report(root, sections):
    path = root / REPORT
    path.parent.mkdir(parents=True, exist_ok=True)
    text = "\n\n".join(f"## {title}\n\n{body.strip()}" for title, body in sections if body)
    path.write_text(f"# Upstream sync report\n\n{text}\n", encoding="utf-8")
    print(text)
    print(f"\nReport: {path}")


def merge_target(root):
    """The upstream commit being merged, pinned: MERGE_HEAD while merging, else HEAD's second parent."""
    head = git(root, "rev-parse", "-q", "--verify", "MERGE_HEAD", check=False).strip()
    if head:
        return head, True
    parents = git(root, "log", "-1", "--format=%P").split()
    if len(parents) == 2:
        return parents[1], False
    raise SyncError("no merge in progress and HEAD is not a merge: nothing to finish")


def finish(root, sections, notes, tests):
    """CLAUDE copy, footprint, commit, tests: everything after the conflicts are gone."""
    sha, merging = merge_target(root)
    if unmerged(root):
        raise SyncError("conflicts remain; resolve and `git add` them, then rerun with --continue")
    # The checks read the working tree and the commit takes the index: they must agree.
    loose = git(root, "diff", "--name-only").split() + git(root, "ls-files", "--others", "--exclude-standard").split()
    if loose:
        raise SyncError("unstaged or untracked changes; `git add` or remove them, then --continue: " + ", ".join(loose[:10]))
    changed = refresh_claude_copy(root, sha)
    sections.append(("Upstream CLAUDE.md", "Changed: read the new rules in `git diff HEAD -- docs/upstream/CLAUDE.md`"
                     " and adopt any worth having in our CLAUDE.md." if changed else "Unchanged."))
    old, original = move_reviewed_base(root, sha)
    ok, out = footprint(root, sha)
    shared = "\n".join(line for line in out.splitlines() if not line.startswith("HOOK"))
    sections.append(("Footprint", f"reviewed_base {old[:7]} -> {sha[:7]}\n\n```\n{shared.strip()}\n```"))
    if not ok:
        (root / MANIFEST).write_bytes(original)
        sections.append(("Stopped", f"A shared budget or hook failed with the new base; reviewed_base stays {old[:7]}. "
                         "Fix the merge, or raise the budget in docs/fork-footprint.json with a reason, "
                         "`git add` it, then rerun with --continue."))
        return 3
    git(root, "add", "--", MANIFEST)
    if merging:
        log = git(root, "log", "--oneline", "--no-merges", f"HEAD..{sha}")
        msg = (f"Merge upstream {UPSTREAM} {sha[:7]} into {git(root, 'branch', '--show-current').strip()}\n\n"
               f"Upstream commits:\n{log}\n" + ("Resolutions:\n" + "\n".join(f"- {n}" for n in notes) + "\n" if notes else "")
               + "docs/upstream/CLAUDE.md refreshed; reviewed_base moved (footprint PASS).\n")
        subprocess.run(["git", "-c", "safe.directory=*", "-C", str(root), "commit", "-q", "-F", "-"],
                       input=msg, text=True, encoding="utf-8", check=True)
    elif git(root, "diff", "--cached", "--name-only").strip():
        git(root, "commit", "-q", "-m", f"sync: refresh upstream CLAUDE.md copy, reviewed_base {sha[:7]}")
    sections.append(("Commit", git(root, "log", "--oneline", "-1").strip()))
    if not tests:
        sections.append(("Tests", "Skipped (--no-tests)."))
        return 0
    code, summary = test_step(root)
    sections.append(("Tests", summary))
    return code


def start(args):
    main_root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    if git(main_root, "config", "merge.ours.driver", check=False).strip() != "true":
        git(main_root, "config", "merge.ours.driver", "true")
    git(main_root, "config", "rerere.enabled", "true")
    git(main_root, "fetch", "origin")
    sha = git(main_root, "rev-parse", UPSTREAM + "^{commit}").strip()  # pinned: a later fetch cannot move it
    count = int(git(main_root, "rev-list", "--count", f"{args.develop}..{sha}").strip())
    if count == 0:
        print(f"Nothing to sync: {args.develop} already contains {UPSTREAM}.")
        return 0
    date = args.date or datetime.date.today().isoformat()
    branch = f"sync/{date}"
    worktree = Path(args.worktree) if args.worktree else main_root.parent / f"eqoa-sync-{date}"
    if worktree.exists() or git(main_root, "branch", "--list", branch).strip():
        raise SyncError(f"{branch} or {worktree} already exists; pass --date or remove it")
    git(main_root, "worktree", "add", "-q", "-b", branch, str(worktree), args.develop)
    print(f"Worktree {worktree} on {branch}; merging {count} upstream commit(s) to {sha[:7]}.")
    merged = subprocess.run(["git", "-c", "safe.directory=*", "-c", "merge.conflictStyle=diff3", "-C", str(worktree),
                             "merge", "--no-ff", "--no-commit", sha],
                            capture_output=True, text=True, encoding="utf-8", errors="replace")
    if not git(worktree, "rev-parse", "-q", "--verify", "MERGE_HEAD", check=False).strip():
        raise SyncError(f"the merge did not start: {merged.stderr.strip() or merged.stdout.strip()}")
    replayed = set(REPLAYED.findall(merged.stdout + merged.stderr))
    notes, sections = [], [("Branch", f"{branch} in {worktree}\n{count} upstream commit(s) to {sha[:7]}:\n\n```\n"
                                       f"{git(worktree, 'log', '--oneline', '--no-merges', f'{args.develop}..{sha}')}```")]
    left = auto_resolve(worktree, notes, replayed)
    sections.append(("Automatic resolutions", "\n".join(f"- {n}" for n in notes) or "None needed."))
    if left:
        sections.append(("Needs a person", "\n".join(f"- {f}" for f in left) +
                         f"\n\nResolve these in {worktree} (`git add` each), then run "
                         "`python tools/sync_upstream.py --continue` there. Git records each resolution "
                         "(rerere) so the same conflict resolves itself next time."))
        write_report(worktree, sections)
        return 2
    code = finish(worktree, sections, notes, not args.no_tests)
    write_report(worktree, sections)
    return code


def resume(args):
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    if not git(root, "branch", "--show-current").startswith("sync/"):
        raise SyncError("--continue runs inside a sync/<date> worktree")
    if args.tests_only:
        code, summary = test_step(root)
        write_report(root, [("Tests", summary)])
        return code
    sections, notes = [], []
    if unmerged(root):
        left = auto_resolve(root, notes)
        if left:
            write_report(root, [("Needs a person", "\n".join(f"- {f}" for f in left))])
            return 2
    notes.append("conflicts resolved by hand (recorded by rerere for next time)")
    code = finish(root, sections, notes, not args.no_tests)
    write_report(root, sections)
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--continue", dest="resume", action="store_true", help="finish a sync after hand resolution")
    parser.add_argument("--tests-only", action="store_true", help="with --continue: only run the tests")
    parser.add_argument("--no-tests", action="store_true", help="skip cargo and Python tests")
    parser.add_argument("--date", help="branch date (default today): sync/<date>")
    parser.add_argument("--worktree", help="worktree folder (default ../eqoa-sync-<date>)")
    parser.add_argument("--develop", default="develop", help="branch to sync from (default develop)")
    args = parser.parse_args()
    try:
        return resume(args) if args.resume else start(args)
    except (SyncError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 4


if __name__ == "__main__":
    sys.exit(main())
