#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Print a one-page summary of what an owner test package recorded.

Reads (never copies or changes) a package made by make_owner_test_package.py: the build
and server, then for the current run and each run set aside in results/<date-time>/ the
fps lines (lowest, typical, worst frame), the loot/merchant/bank opcode lines, mod
errors, recording metadata and crash files, and any F3 report zips on the Desktop newer than the package.
Account files are never read. Paths print relative to the package; a local user-profile
path in a log line is replaced by `~`.
"""
import argparse
import json
import re
import statistics
import sys
from pathlib import Path

if __package__:
    from .rec_dump import records
else:
    from rec_dump import records

FPS_LINE = re.compile(r"perf: ([0-9.]+) fps.*?\(worst ([0-9.]+)")
OPCODE_LINE = re.compile(r"play: (0x[0-9a-fA-F]{4}) \((\d+) bytes\) in mode (\S+)")
LOCAL_PATH = re.compile(r"[A-Za-z]:[\\/]+Users[\\/]+[^\\/\s\"']+")
ACCOUNT_FILE = re.compile(r"^(account.*|acct_.*)\.opt$", re.IGNORECASE)
OPCODE_SHOWN = 12
ERROR_LINES_SHOWN = 5


def scrub(text, package):
    text = text.replace(str(package), "<package>")
    return LOCAL_PATH.sub("~", text)


def read_lines(path):
    if ACCOUNT_FILE.match(path.name):
        return []
    try:
        return path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return []


def build_line(package):
    try:
        manifest = json.loads((package / "play" / "release-manifest.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return "build: unknown (no play/release-manifest.json)"
    commit = str(manifest.get("sourceCommit", "unknown"))[:12]
    return f"build: {manifest.get('version', '?')} commit {commit} channel {manifest.get('updaterChannel', '?')}"


def server_line(package):
    lines = read_lines(package / "config" / "server.opt")
    host = lines[0].strip().split()[0] if lines and lines[0].strip() else ""
    if not host or "@" in host:
        return "server: unknown"
    return f"server: {host}"


def run_dirs(package):
    """(label, folder holding config/ and play/ files) for the current run, then older ones, newest first."""
    runs = [("current run", package)]
    results = package / "results"
    if results.is_dir():
        for folder in sorted((d for d in results.iterdir() if d.is_dir()), key=lambda d: d.name, reverse=True):
            runs.append((f"set aside {folder.name}", folder))
    return runs


def fps_summary(lines):
    fps, worst = [], []
    for line in lines:
        match = FPS_LINE.search(line)
        if match:
            fps.append(float(match.group(1)))
            worst.append(float(match.group(2)))
    if not fps:
        return "fps: no perf lines (use the fps log launcher)"
    return (
        f"fps: {len(fps)} readings, lowest {min(fps):.1f}, typical {statistics.median(fps):.1f}, "
        f"worst frame {max(worst):.1f} ms"
    )


def opcode_summary(lines):
    seen = [m.groups() for m in map(OPCODE_LINE.search, lines) if m]
    if not seen:
        return ["opcodes: none logged"]
    counts = {}
    for op, _size, _mode in seen:
        counts[op] = counts.get(op, 0) + 1
    out = ["opcodes: " + ", ".join(f"{op} x{n}" for op, n in sorted(counts.items()))]
    shown = [f"{op}({size}b)" for op, size, _mode in seen[:OPCODE_SHOWN]]
    more = " ..." if len(seen) > OPCODE_SHOWN else ""
    out.append("  order: " + " ".join(shown) + more)
    return out


def files_named(folder, test):
    if not folder.is_dir():
        return []
    skip = folder / "results"
    return sorted(
        p for p in folder.rglob("*")
        if p.is_file() and test(p.name) and not ACCOUNT_FILE.match(p.name) and skip not in p.parents
    )


def recording_summaries(package, base):
    paths = sorted(p for p in (base / "config").glob("session-*") if p.suffix.lower() == ".eqrec" and p.is_file())
    if not paths:
        return ["recordings: none"]
    out = []
    for path in paths:
        label = path.relative_to(package).as_posix()
        try:
            with path.open("rb") as stream:
                size = path.stat().st_size
                counts = [0, 0]
                first = last = 0
                try:
                    for micros, direction, _, _ in records(stream):
                        if not any(counts):
                            first = micros
                        last = micros
                        counts[direction] += 1
                    detail = f"{(last - first) / 1e6:.3f} s, {sum(counts)} messages, {counts[0]} received, {counts[1]} sent"
                except ValueError as error:
                    detail = str(error)
        except OSError:
            out.append(f"recording: {label} (unreadable)")
            continue
        out.append(f"recording: {label} ({size} bytes, {detail})")
    return out


def run_summary(package, label, base):
    out = [f"== {label} =="]
    logs = files_named(base, lambda n: n == "eqoa.log")
    if not logs:
        out.append("log: none")
    for log in logs:
        lines = read_lines(log)
        out.append(f"log: {log.relative_to(package).as_posix()} ({len(lines)} lines)")
        out.append(fps_summary(lines))
        out.extend(opcode_summary(lines))
    out.extend(recording_summaries(package, base))
    errors = files_named(base, lambda n: n.startswith("addons_error.log"))
    if not errors:
        out.append("mod errors: none")
    for path in errors:
        lines = [l for l in read_lines(path) if l.strip()]
        out.append(f"mod errors: {path.relative_to(package).as_posix()} ({len(lines)} lines)")
        out.extend("  " + line for line in lines[-ERROR_LINES_SHOWN:])
    crashes = files_named(base, lambda n: n.startswith("crash") and n.endswith(".txt"))
    crashes = [c for c in crashes if "crash" in c.parent.name or c.parent.name == "crashes"]
    if crashes:
        out.append("crash files: " + ", ".join(c.relative_to(package).as_posix() for c in crashes))
    else:
        out.append("crash files: none")
    return out


def package_time(package):
    stamps = [p.stat().st_mtime for p in package.glob("*.bat")]
    return max(stamps) if stamps else package.stat().st_mtime


def f3_zips(package, desktop):
    if desktop is None or not Path(desktop).is_dir():
        return []
    since = package_time(package)
    return sorted(p.name for p in Path(desktop).glob("*.zip") if p.stat().st_mtime > since)


def collect(package, desktop=None):
    package = Path(package).resolve()
    if not package.is_dir():
        raise FileNotFoundError("Package folder not found")
    out = ["EQOA Enhanced owner test summary", build_line(package), server_line(package)]
    checklist = package / "CHECKLIST.txt"
    out.append("checklist: " + ("CHECKLIST.txt in the package" if checklist.is_file() else "none"))
    for label, base in run_dirs(package):
        out.extend(run_summary(package, label, base))
    zips = f3_zips(package, desktop)
    out.append("== F3 reports on the Desktop newer than the package ==")
    out.append(", ".join(zips) if zips else "none")
    return scrub("\n".join(out), package) + "\n"


def default_desktop():
    return Path.home() / "Desktop"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path, help="the owner test package folder")
    parser.add_argument("--desktop", type=Path, help="where F3 report zips are looked for (default: the user's Desktop)")
    args = parser.parse_args(argv)
    try:
        sys.stdout.write(collect(args.package, args.desktop or default_desktop()))
    except (OSError, ValueError) as error:
        parser.exit(1, f"Error: {scrub(str(error), args.package)}\n")


if __name__ == "__main__":
    main()
