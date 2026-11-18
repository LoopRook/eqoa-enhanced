#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Run the development gates on GitHub's runners instead of this PC.

push <branch>      pushes a local branch as ci/<branch> to looprook and prints the run URL
status <branch>    waits for that run and prints pass/fail per gate, SKIPs and failing tests
cleanup <branch>   deletes the remote ci/<branch>
cleanup --merged   deletes every remote ci/* branch already merged into develop
summarize ...      (runs inside the workflow) turns gate logs into a short summary

The only refs this tool writes to are refs/heads/ci/*: every other ref is refused before
any git command is built. See docs/DEV-TOOLS.md.
"""
import argparse
import json
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from run_gates import parse_cargo_test_output

ROOT = Path(__file__).resolve().parents[1]
REMOTE = "looprook"
REPO = "LoopRook/eqoa-enhanced"
WORKFLOW = "branch-gates.yml"
PREFIX = "ci/"
REF_PREFIX = "refs/heads/ci/"
SUMMARY_JOBS = ("tests", "checks")

# Letters, digits and . _ - / only: no ':', '*', '^', '~', '?', '[', '\', spaces or '@{'.
BRANCH_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$")


class RefusedRef(ValueError):
    pass


def ci_branch(name):
    """The remote branch name `ci/<name>` for a local branch (or an existing ci/ name).

    Anything that is not a plain branch name (a tag, a full ref, a refspec, a path
    trick, an empty name) raises RefusedRef."""
    if not isinstance(name, str):
        raise RefusedRef("branch name must be text")
    name = name.strip()
    if name.startswith("refs/heads/"):
        name = name[len("refs/heads/"):]
    if name.startswith(PREFIX):
        name = name[len(PREFIX):]
    if (
        not BRANCH_RE.match(name)
        or name.split("/")[0] in ("refs", "HEAD")
        or ".." in name
        or "//" in name
        or name.endswith(("/", ".", ".lock"))
        or any(part.startswith(".") or part.endswith(".lock") for part in name.split("/"))
    ):
        raise RefusedRef(f"refusing {name!r}: not a plain branch name")
    return PREFIX + name


def local_ref(name):
    name = name.strip()
    if name.startswith("refs/heads/"):
        name = name[len("refs/heads/"):]
    ci_branch(name)  # same validation as the destination
    return "refs/heads/" + name


def require_ci_ref(ref):
    """Last guard before a git write: the ref must be under refs/heads/ci/."""
    if not (isinstance(ref, str) and ref.startswith(REF_PREFIX) and len(ref) > len(REF_PREFIX)):
        raise RefusedRef(f"refusing to write {ref!r}: only {REF_PREFIX}* may be written")
    ci_branch(ref[len("refs/heads/"):])
    return ref


def push_command(branch):
    destination = require_ci_ref("refs/heads/" + ci_branch(branch))
    return ["git", "-c", "safe.directory=*", "push", "--force", REMOTE, f"{local_ref(branch)}:{destination}"]


def delete_command(branch):
    destination = require_ci_ref("refs/heads/" + ci_branch(branch))
    return ["git", "-c", "safe.directory=*", "push", REMOTE, "--delete", destination]


def run(cmd, check=True):
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if check and result.returncode:
        raise SystemExit(f"{' '.join(cmd[:4])} failed: {(result.stderr or result.stdout).strip()}")
    return result


def git(*args, check=True):
    return run(["git", "-c", "safe.directory=*", *args], check=check)


def remote_sha(branch):
    out = git("ls-remote", REMOTE, require_ci_ref("refs/heads/" + ci_branch(branch))).stdout.split()
    return out[0] if out else None


def runs_for(branch, sha=None):
    out = run(["gh", "run", "list", "--repo", REPO, "--workflow", WORKFLOW, "--branch", ci_branch(branch),
               "--limit", "10", "--json", "databaseId,headSha,url,status,conclusion,createdAt"]).stdout
    runs = json.loads(out or "[]")
    return [r for r in runs if sha is None or r["headSha"] == sha]


def find_run(branch, sha, wait=90):
    deadline = time.time() + wait
    while True:
        runs = runs_for(branch, sha)
        if runs:
            return runs[0]
        if time.time() >= deadline:
            return None
        time.sleep(5)


def cmd_push(args):
    branch = args.branch
    command = push_command(branch)
    sha = git("rev-parse", "--verify", local_ref(branch)).stdout.strip()
    git(*command[3:])
    print(f"pushed {branch} ({sha[:10]}) as {ci_branch(branch)}")
    found = find_run(branch, sha)
    if found:
        print(f"run: {found['url']}")
    else:
        print(f"no run appeared for {sha[:10]} (an unchanged commit starts no new run); see "
              f"https://github.com/{REPO}/actions/workflows/{WORKFLOW}")
    return 0


def format_status(jobs, summaries):
    lines = []
    for job in jobs:
        lines.append(f"{job['name']}: {job.get('conclusion') or job['status']}")
        for step in job.get("steps", []):
            if step["name"] in ("addons", "full", "wasm", "footprint", "paths", "player-release", "tools"):
                lines.append(f"  {step['name']}: {step.get('conclusion') or step['status']}")
    for summary in summaries:
        for gate in summary.get("gates", []):
            bits = [f"{gate['name']}: {gate['outcome']}"]
            if gate.get("passed") is not None:
                bits.append(f"{gate['passed']} passed, {gate['failed']} failed, {gate['ignored']} ignored")
            if gate.get("skips"):
                bits.append(f"{gate['skips']} SKIP")
            lines.append("  " + "; ".join(bits))
            for name in gate.get("failing", []):
                lines.append(f"    FAILING: {name}")
    return "\n".join(lines)


def fetch_summaries(run_id):
    summaries = []
    for job in SUMMARY_JOBS:
        with tempfile.TemporaryDirectory() as tmp:
            result = run(["gh", "run", "download", str(run_id), "--repo", REPO, "-n", f"gate-summary-{job}", "-D", tmp],
                         check=False)
            path = Path(tmp) / f"gate-summary-{job}.json"
            if result.returncode == 0 and path.exists():
                summaries.append(json.loads(path.read_text(encoding="utf-8")))
    return summaries


def cmd_status(args):
    sha = remote_sha(args.branch)
    if sha is None:
        raise SystemExit(f"{ci_branch(args.branch)} is not on {REMOTE}; run push first")
    found = find_run(args.branch, sha, wait=0 if args.no_wait else 90)
    if not found:
        raise SystemExit(f"no run for {sha[:10]} on {ci_branch(args.branch)}")
    started = time.time()
    while True:
        view = json.loads(run(["gh", "run", "view", str(found["databaseId"]), "--repo", REPO, "--json",
                               "status,conclusion,url,createdAt,updatedAt,jobs"]).stdout)
        if view["status"] == "completed" or args.no_wait:
            break
        if time.time() - started > args.timeout * 60:
            raise SystemExit(f"timed out after {args.timeout} minutes; run still {view['status']}: {view['url']}")
        print(f"... {view['status']} ({int(time.time() - started)}s)", flush=True)
        time.sleep(args.poll)
    print(f"run: {view['url']}")
    print(f"result: {view.get('conclusion') or view['status']}  started {view['createdAt']}  ended {view['updatedAt']}")
    print(format_status(view.get("jobs", []), fetch_summaries(found["databaseId"]) if view["status"] == "completed" else []))
    return 0 if view.get("conclusion") == "success" else 1


def cmd_cleanup(args):
    if args.merged == bool(args.branch):
        raise SystemExit("give a branch, or --merged")
    if args.branch:
        branches = [args.branch]
    else:
        listing = git("ls-remote", "--heads", REMOTE, REF_PREFIX + "*").stdout.splitlines()
        develop = git("rev-parse", "--verify", "-q", "refs/heads/develop", check=False).stdout.strip() \
            or git("rev-parse", "--verify", "-q", f"refs/remotes/{REMOTE}/develop", check=False).stdout.strip()
        if not develop:
            raise SystemExit("cannot find develop locally to compare against")
        branches = []
        for line in listing:
            sha, ref = line.split()[:2]
            try:
                name = ci_branch(ref[len("refs/heads/"):])
            except RefusedRef:
                continue
            if git("cat-file", "-e", sha + "^{commit}", check=False).returncode:
                git("fetch", "--no-tags", REMOTE, ref, check=False)
            merged = git("merge-base", "--is-ancestor", sha, develop, check=False).returncode == 0
            print(f"{name}: {'merged into develop' if merged else 'not merged, kept'}")
            if merged:
                branches.append(name)
    for branch in branches:
        command = delete_command(branch)
        if args.dry_run:
            print("would run:", " ".join(command))
            continue
        result = run(command, check=False)
        if result.returncode:
            print(f"could not delete {ci_branch(branch)}: {(result.stderr or result.stdout).strip()}")
            return 1
        print(f"deleted {ci_branch(branch)}")
    return 0


def read_log(path):
    if not path.exists():
        return ""
    data = path.read_bytes()
    if data[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return data.decode("utf-16", errors="replace")
    return data.decode("utf-8-sig", errors="replace")


def parse_gate_log(text):
    """passed/failed/ignored, SKIP count and failing names from a cargo test or unittest log."""
    cargo = parse_cargo_test_output(text)
    if cargo["found_test_results"]:
        return {"passed": cargo["passed"], "failed": cargo["failed"], "ignored": cargo["ignored"],
                "skips": cargo["skip_count"], "failing": cargo["failed_names"][:50]}
    ran = re.search(r"^Ran (\d+) tests?", text, re.MULTILINE)
    if ran:
        total = int(ran.group(1))
        failing = list(dict.fromkeys(re.findall(r"^(?:FAIL|ERROR): (.+)$", text, re.MULTILINE)))
        skipped = re.search(r"skipped=(\d+)", text)
        skipped = int(skipped.group(1)) if skipped else 0
        return {"passed": total - len(failing) - skipped, "failed": len(failing), "ignored": skipped,
                "skips": sum("SKIP" in line for line in text.splitlines()), "failing": failing[:50]}
    return {"passed": None, "failed": None, "ignored": None, "skips": 0, "failing": []}


def summarize(job, logs, results):
    gates = []
    for name, outcome in results:
        gate = {"name": name, "outcome": outcome or "unknown"}
        gate.update(parse_gate_log(read_log(Path(logs) / f"{name}.log")))
        gates.append(gate)
    return {"job": job, "gates": gates, "ok": all(g["outcome"] == "success" for g in gates)}


def markdown(summary):
    lines = [f"### Branch gates: {summary['job']} ({'PASS' if summary['ok'] else 'FAIL'})", "",
             "| gate | result | passed | failed | ignored | SKIP lines |", "|---|---|---|---|---|---|"]
    for gate in summary["gates"]:
        cells = ["" if gate[key] is None else gate[key] for key in ("passed", "failed", "ignored")]
        lines.append(f"| {gate['name']} | {gate['outcome']} | " + " | ".join(map(str, cells)) + f" | {gate['skips']} |")
    for gate in summary["gates"]:
        for name in gate["failing"]:
            lines.append(f"\n- failing in {gate['name']}: `{name}`")
    return "\n".join(lines) + "\n"


def cmd_summarize(args):
    results = []
    for item in args.result:
        name, _, outcome = item.partition("=")
        results.append((name, outcome))
    summary = summarize(args.job, args.logs, results)
    Path(args.out).write_text(json.dumps(summary, indent=2), encoding="utf-8")
    text = markdown(summary)
    print(text)
    if args.markdown:
        with open(args.markdown, "a", encoding="utf-8") as handle:
            handle.write(text)
    return 0


def build_parser():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    push = sub.add_parser("push", help="push a local branch as ci/<branch> and print the run URL")
    push.add_argument("branch")
    push.set_defaults(func=cmd_push)
    status = sub.add_parser("status", help="wait for the run on ci/<branch> and print the result")
    status.add_argument("branch")
    status.add_argument("--no-wait", action="store_true", help="print the current state and return")
    status.add_argument("--timeout", type=int, default=120, help="minutes to wait (default 120)")
    status.add_argument("--poll", type=int, default=30, help="seconds between checks (default 30)")
    status.set_defaults(func=cmd_status)
    cleanup = sub.add_parser("cleanup", help="delete remote ci/ branches")
    cleanup.add_argument("branch", nargs="?")
    cleanup.add_argument("--merged", action="store_true", help="every ci/* branch already merged into develop")
    cleanup.add_argument("--dry-run", action="store_true")
    cleanup.set_defaults(func=cmd_cleanup)
    summarize_parser = sub.add_parser("summarize", help="(in the workflow) summarize gate logs")
    summarize_parser.add_argument("--job", required=True)
    summarize_parser.add_argument("--logs", required=True)
    summarize_parser.add_argument("--out", required=True)
    summarize_parser.add_argument("--markdown")
    summarize_parser.add_argument("--result", action="append", default=[], help="gate=outcome")
    summarize_parser.set_defaults(func=cmd_summarize)
    return parser


def main(argv=None):
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except RefusedRef as error:
        print(f"refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
