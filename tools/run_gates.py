#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Run workspace gates in order and generate a short summary report."""
import argparse
import csv
import datetime
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time

DEFAULT_FULL_SKIPS = [
    "round4_",
    "frame_time",
    "benchmark",
    "scrollback_10000",
    "measure",
]

TEST_RESULT_RE = re.compile(
    r"test result:\s+(?:ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored;"
)
TEST_LINE_RE = re.compile(r"^test\s+\S+\s+\.\.\.\s+(?:ok|FAILED|ignored)\b")
FAILED_TEST_RE = re.compile(r"^test\s+([^\s]+)\s+\.\.\.\s+FAILED", re.MULTILINE)


VALID_STEP_NAMES = [
    "round",
    "addons",
    "full",
    "wasm",
    "footprint",
    "paths",
    "player-release",
    "release",
]


def safe_decode(data):
    if data is None:
        return ""
    if isinstance(data, bytes):
        return data.decode("utf-8", errors="replace")
    return str(data)


def kill_process_tree(pid):
    if pid is None:
        return
    if sys.platform == "win32":
        try:
            subprocess.run(
                ["taskkill", "/T", "/F", "/PID", str(pid)],
                capture_output=True,
                check=False,
            )
        except OSError:
            pass
    else:
        try:
            import signal
            pgid = os.getpgid(pid)
            os.killpg(pgid, signal.SIGKILL)
        except (OSError, ProcessLookupError, AttributeError):
            try:
                import signal
                os.kill(pid, signal.SIGKILL)
            except (OSError, ProcessLookupError):
                pass


def _format_progress_line(step_name, elapsed_seconds, is_test, test_count):
    label = step_name or "step"
    elapsed_minutes = elapsed_seconds / 60.0
    if is_test:
        return f"  [{label}] {elapsed_minutes:.1f}m elapsed ({test_count} tests)"
    return f"  [{label}] {elapsed_minutes:.1f}m elapsed"


def _stream_subprocess_output(proc, timeout_seconds, step_name, is_test, progress_interval):
    line_queue = queue.Queue()
    reader_error = []

    def _reader():
        try:
            for raw_line in iter(proc.stdout.readline, b""):
                line_queue.put(raw_line)
        except Exception as exc:
            reader_error.append(exc)
        finally:
            line_queue.put(None)

    reader = threading.Thread(target=_reader, daemon=True)
    reader.start()

    start_time = time.monotonic()
    deadline = start_time + timeout_seconds
    next_progress = start_time + progress_interval
    chunks = []
    test_count = 0
    timed_out = False
    eof = False

    try:
        while not eof:
            now = time.monotonic()
            if now >= deadline:
                timed_out = True
                kill_process_tree(proc.pid)
                break

            wait_for = max(0.01, min(next_progress, deadline) - now)
            try:
                item = line_queue.get(timeout=wait_for)
            except queue.Empty:
                item = False  # sentinel for timeout on queue.get

            if item is None:
                eof = True
                break
            if item is not False:
                chunks.append(item)
                if is_test and TEST_LINE_RE.match(safe_decode(item)):
                    test_count += 1

            now = time.monotonic()
            if now >= next_progress:
                print(
                    _format_progress_line(step_name, now - start_time, is_test, test_count),
                    flush=True,
                )
                while next_progress <= now:
                    next_progress += progress_interval

        if reader_error:
            kill_process_tree(proc.pid)
            err_msg = f"Stream reader error for {step_name or 'step'}: {reader_error[0]}"
            print(err_msg, file=sys.stderr, flush=True)
            chunks.append(f"\n{err_msg}\n".encode("utf-8", errors="replace"))
            try:
                proc.wait(timeout=5)
            except Exception:
                pass
        elif timed_out:
            try:
                proc.wait(timeout=5)
            except Exception:
                pass
        else:
            try:
                proc.wait(timeout=max(0.01, deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                timed_out = True
                kill_process_tree(proc.pid)
                try:
                    proc.wait(timeout=5)
                except Exception:
                    pass

        reader.join(timeout=2)
        if not reader.is_alive():
            try:
                proc.stdout.close()
            except Exception:
                pass
        while True:
            try:
                item = line_queue.get_nowait()
            except queue.Empty:
                break
            if item is not None:
                chunks.append(item)
    except BaseException:
        kill_process_tree(proc.pid)
        try:
            proc.wait(timeout=5)
        except Exception:
            pass
        raise

    returncode = -1 if (timed_out or reader_error) else proc.returncode
    return returncode, b"".join(chunks), timed_out


def run_subprocess_tree(
    cmd,
    cwd,
    env,
    timeout_minutes,
    *,
    step_name=None,
    is_test=False,
    progress_interval=None,
):
    timeout_seconds = timeout_minutes * 60
    proc = subprocess.Popen(
        cmd,
        cwd=str(cwd),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        start_new_session=(sys.platform != "win32"),
    )
    timed_out = False
    stdout_data = None
    if progress_interval is not None and progress_interval > 0:
        returncode, stdout_data, timed_out = _stream_subprocess_output(
            proc,
            timeout_seconds,
            step_name,
            is_test,
            progress_interval,
        )
    else:
        try:
            stdout_data, _ = proc.communicate(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            timed_out = True
            kill_process_tree(proc.pid)
            try:
                stdout_data, _ = proc.communicate()
            except Exception:
                stdout_data = exc.stdout
        except BaseException:
            kill_process_tree(proc.pid)
            try:
                proc.communicate()
            except Exception:
                pass
            raise
        returncode = proc.returncode

    stdout_text = safe_decode(stdout_data)
    if timed_out:
        stdout_text += f"\nCommand timed out after {timeout_minutes} minutes\n"
        returncode = -1

    return returncode, stdout_text


def get_current_branch(root=None):
    try:
        cmd = ["git", "-c", "safe.directory=*", "rev-parse", "--abbrev-ref", "HEAD"]
        if root:
            cmd = ["git", "-c", "safe.directory=*", "-C", str(root), "rev-parse", "--abbrev-ref", "HEAD"]
        output = subprocess.check_output(cmd, stderr=subprocess.DEVNULL, text=True).strip()
        if output:
            return output
    except Exception:
        pass
    return "unknown"


def sanitize_branch_name(branch):
    return re.sub(r"[^A-Za-z0-9._-]+", "-", branch).strip("-") or "branch"


def read_reviewed_base(root):
    path = Path(root) / "docs" / "fork-footprint.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    return data["reviewed_base"]


def parse_cargo_test_output(text):
    total_passed = 0
    total_failed = 0
    total_ignored = 0
    found_any = False

    for match in TEST_RESULT_RE.finditer(text):
        found_any = True
        total_passed += int(match.group(1))
        total_failed += int(match.group(2))
        total_ignored += int(match.group(3))

    failed_names = []
    for match in FAILED_TEST_RE.finditer(text):
        name = match.group(1)
        if name not in failed_names:
            failed_names.append(name)

    in_failures = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == "failures:":
            in_failures = True
            continue
        if in_failures:
            if not stripped or stripped.startswith("test result:"):
                in_failures = False
            elif line.startswith("    ") and not line.startswith("    ----"):
                name = stripped.split()[0]
                if name and name not in failed_names:
                    failed_names.append(name)

    skip_lines = []
    for line in text.splitlines():
        if "SKIP" in line:
            skip_lines.append(line.strip())

    skip_count = len(skip_lines)
    distinct_skips = list(dict.fromkeys(skip_lines))[:30]

    return {
        "found_test_results": found_any,
        "passed": total_passed,
        "failed": total_failed,
        "ignored": total_ignored,
        "failed_names": failed_names,
        "skip_count": skip_count,
        "distinct_skips": distinct_skips,
    }


def process_alive(pid):
    """True while process `pid` runs. Unsure answers count as alive, so a lock is never wrongly removed."""
    if sys.platform == "win32":
        import ctypes

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        handle = kernel32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
        if not handle:
            # 87 = ERROR_INVALID_PARAMETER: no such process; anything else (access denied) means it exists
            return ctypes.get_last_error() != 87
        try:
            code = ctypes.c_ulong()
            if not kernel32.GetExitCodeProcess(handle, ctypes.byref(code)):
                return True
            return code.value == 259  # STILL_ACTIVE
        finally:
            kernel32.CloseHandle(handle)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except OSError:
        return True
    return True


def cargo_process_running():
    """Treat a failed process listing as busy."""
    try:
        if sys.platform == "win32":
            result = subprocess.run(
                ["tasklist", "/fo", "csv", "/nh"],
                capture_output=True, text=True, check=True, timeout=10,
            )
            rows = list(csv.reader(result.stdout.splitlines(), strict=True))
            if any(len(row) != 5 or not row[1].isdecimal() for row in rows):
                return True
            names = [row[0].lower() for row in rows]
        else:
            result = subprocess.run(
                ["ps", "-A", "-o", "comm="],
                capture_output=True, text=True, check=True, timeout=10,
            )
            names = [Path(line.strip()).name for line in result.stdout.splitlines()]
    except (OSError, subprocess.SubprocessError, UnicodeError, csv.Error):
        return True
    return not names or any(name in {"cargo", "rustc", "cargo.exe", "rustc.exe"} for name in names)


class CargoLock:
    EMPTY_STALE_SECONDS = 5 * 60

    def __init__(self, lock_dir=None, wait_minutes=60, poll_interval=30):
        if lock_dir is None:
            lock_dir = Path.home() / "Paseo" / "cargo-build.lock"
        self.lock_dir = Path(lock_dir)
        self.wait_minutes = wait_minutes
        self.poll_interval = poll_interval
        self.owner_token = f"pid-{os.getpid()}-{id(self)}.token"
        self.acquired = False

    def acquire(self):
        timeout_seconds = self.wait_minutes * 60
        start = time.time()
        self.lock_dir.parent.mkdir(parents=True, exist_ok=True)
        while True:
            try:
                os.mkdir(self.lock_dir)
            except FileExistsError:
                if self._clear_if_holder_dead():
                    continue
                elapsed = time.time() - start
                if elapsed >= timeout_seconds:
                    raise TimeoutError(
                        f"Timed out after {self.wait_minutes} minutes waiting for cargo lock at {self.lock_dir}"
                    )
                holder_pid = self._holder_pid()
                holder_note = f", held by pid {holder_pid}" if holder_pid is not None else ""
                print(
                    f"Waiting for cargo lock at {self.lock_dir}... ({int(elapsed)}s elapsed{holder_note})",
                    flush=True,
                )
                time.sleep(min(self.poll_interval, max(0.1, timeout_seconds - elapsed)))
                continue
            except PermissionError:
                # Windows refuses mkdir while another holder is removing the folder: wait, never
                # remove a lock this process did not create.
                if time.time() - start >= timeout_seconds:
                    raise TimeoutError(
                        f"Timed out after {self.wait_minutes} minutes waiting for cargo lock at {self.lock_dir}"
                    )
                time.sleep(min(self.poll_interval, 1.0))
                continue

            try:
                (self.lock_dir / self.owner_token).touch()
            except OSError:
                # Running without a token would look stale to a later waiter.
                self.lock_dir.rmdir()
                raise
            self.acquired = True
            return

    def _holder_pid(self):
        try:
            tokens = [p for p in self.lock_dir.iterdir() if p.name.startswith("pid-") and p.name.endswith(".token")]
        except OSError:
            return None
        if len(tokens) != 1:
            return None
        try:
            return int(tokens[0].name.split("-")[1])
        except (IndexError, ValueError):
            return None

    def _clear_if_holder_dead(self):
        """Remove a lock left by a process that no longer runs (a killed gate run).

        An empty lock gets a grace period for its holder to write the token.
        """
        try:
            stamp = self.lock_dir.stat()
            entries = list(self.lock_dir.iterdir())
            if not entries:
                if time.time() - stamp.st_mtime < self.EMPTY_STALE_SECONDS or cargo_process_running():
                    return False
                current = self.lock_dir.stat()
                if not os.path.samestat(stamp, current) or stamp.st_mtime_ns != current.st_mtime_ns:
                    return False
                # rmdir also refuses a directory that gained a token during the check.
                self.lock_dir.rmdir()
                print(f"Removed stale empty cargo lock at {self.lock_dir}")
                return True
            tokens = [p for p in entries if p.name.startswith("pid-") and p.name.endswith(".token")]
        except OSError:
            return False
        if len(tokens) != 1:
            return False
        try:
            pid = int(tokens[0].name.split("-")[1])
        except (IndexError, ValueError):
            return False
        if pid == os.getpid() or process_alive(pid):
            return False
        print(f"Removing stale cargo lock at {self.lock_dir}: holder pid {pid} is no longer running")
        try:
            tokens[0].unlink(missing_ok=True)
            os.rmdir(self.lock_dir)
        except OSError:
            return False
        return True

    def release(self):
        token_path = self.lock_dir / self.owner_token
        if self.acquired or token_path.is_file():
            try:
                token_path.unlink(missing_ok=True)
            except OSError:
                pass
            try:
                os.rmdir(self.lock_dir)
            except OSError:
                pass
            self.acquired = False

    def __enter__(self):
        self.acquire()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.release()


# ---------------------------------------------------------------------------
# Shared-target stale-binary guard.
#
# Every worktree builds into ONE CARGO_TARGET_DIR. Cargo's fingerprint for the eqoa unit is the same
# for all worktrees and its dep-info paths are package-relative, so cargo only compares this
# worktree's source mtimes with the time of the LAST build, whichever worktree made it. If another
# worktree built after this one's last edit, cargo prints "Finished in 0.87s" and runs THAT
# worktree's binary: a gate can pass on someone else's code.
#
# The guard, run while holding the cargo lock: (1) touches every workspace crate's root source file
# (and build.rs) so cargo sees this worktree as newer than any shared artifact, unless the marker
# proves this worktree was the last builder; (2) after the build, requires the tested executable to
# be newer than that touch; (3) records the tested executable and its mtime in the summary.
# ---------------------------------------------------------------------------

GUARD_MARKER_NAME = "last-build.json"
GUARD_TOLERANCE_SECONDS = 1.0
# target-dir sub-paths whose deps/*.d files change on every cargo build of any worktree
ARTIFACT_PROFILE_DIRS = (
    ("debug",),
    ("release",),
    ("wasm32-unknown-unknown", "debug"),
    ("wasm32-unknown-unknown", "release"),
)
RUNNING_EXE_RE = re.compile(r"^\s*Running\s+.*\(([^()]+)\)\s*$", re.MULTILINE)


class StaleBinaryError(RuntimeError):
    """The executable cargo ran was not rebuilt from this worktree's sources."""


def _norm_root(root):
    return os.path.normcase(str(Path(root).resolve()))


def workspace_root_sources(root):
    """Root source files of every workspace crate under crates/: main.rs, lib.rs and build.rs."""
    crates = Path(root) / "crates"
    found = []
    if not crates.is_dir():
        return found
    for crate in sorted(p for p in crates.iterdir() if p.is_dir()):
        for rel in (("src", "main.rs"), ("src", "lib.rs"), ("build.rs",)):
            candidate = crate.joinpath(*rel)
            if candidate.is_file():
                found.append(candidate)
    return found


def touch_workspace_sources(root, now=None):
    """Set the mtime of every workspace crate root file to `now`; return (now, files)."""
    if now is None:
        now = time.time()
    files = workspace_root_sources(root)
    for path in files:
        os.utime(path, (now, now))
    return now, files


def newest_artifact_mtime(target_dir):
    """Newest mtime of any cargo dep-info file in the shared target dir (0.0 when none)."""
    newest = 0.0
    for parts in ARTIFACT_PROFILE_DIRS:
        deps = Path(target_dir).joinpath(*parts, "deps")
        try:
            entries = list(os.scandir(deps))
        except OSError:
            continue
        for entry in entries:
            if entry.name.endswith(".d"):
                try:
                    newest = max(newest, entry.stat().st_mtime)
                except OSError:
                    pass
    return newest


def read_guard_marker(path):
    try:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def guard_needs_touch(marker, root, artifact_stamp):
    """True unless the marker proves this worktree built last and nothing was built since."""
    if not marker:
        return True
    try:
        if marker.get("root") != _norm_root(root):
            return True
        return float(artifact_stamp) > float(marker["stamp"]) + 1e-6
    except (KeyError, TypeError, ValueError):
        return True


def parse_test_exes(text, root=None):
    """Executables cargo test reports on its `Running ... (path)` lines, in order, de-duplicated."""
    exes = []
    for match in RUNNING_EXE_RE.finditer(text):
        path = Path(match.group(1).strip())
        if root is not None and not path.is_absolute():
            path = Path(root) / path
        if path not in exes:
            exes.append(path)
    return exes


def check_exe_fresh(exe, touched_at, tolerance=GUARD_TOLERANCE_SECONDS):
    """(fresh, mtime): fresh when the executable was written no earlier than the touch."""
    try:
        mtime = Path(exe).stat().st_mtime
    except OSError:
        return False, None
    return mtime >= touched_at - tolerance, mtime


def describe_exe(exe, mtime):
    if mtime is None:
        return f"{exe} (missing)"
    stamp = datetime.datetime.fromtimestamp(mtime).strftime("%Y-%m-%d %H:%M:%S")
    return f"{exe} (modified {stamp})"


class SharedTargetGuard:
    """Use inside the CargoLock: begin() before cargo, verify() on the executables, end() after."""

    def __init__(self, root, target_dir):
        self.root = Path(root).resolve()
        self.target_dir = Path(target_dir)
        self.marker_path = self.target_dir / "gates" / GUARD_MARKER_NAME
        self.touched_at = None
        self.touched_now = False

    def begin(self, force=False):
        marker = read_guard_marker(self.marker_path)
        stamp = newest_artifact_mtime(self.target_dir)
        if force or guard_needs_touch(marker, self.root, stamp):
            self.touched_at, _ = touch_workspace_sources(self.root)
            self.touched_now = True
        else:
            self.touched_at = float(marker["touched_at"])
            self.touched_now = False
        return self.touched_at

    def verify(self, exes):
        """Return [(exe, mtime)]; raise StaleBinaryError when none or any was not rebuilt."""
        if not exes:
            raise StaleBinaryError("could not identify the executable cargo ran; cannot prove it is this worktree's")
        results = []
        stale = []
        for exe in exes:
            fresh, mtime = check_exe_fresh(exe, self.touched_at)
            results.append((exe, mtime))
            if not fresh:
                stale.append(describe_exe(exe, mtime))
        if stale:
            touched = datetime.datetime.fromtimestamp(self.touched_at).strftime("%Y-%m-%d %H:%M:%S")
            raise StaleBinaryError(
                f"executable not rebuilt after this worktree's sources were marked new ({touched}): "
                + "; ".join(stale)
                + ". Cargo reused an artifact from another worktree; do not trust this result."
            )
        return results

    def end(self):
        self.marker_path.parent.mkdir(parents=True, exist_ok=True)
        self.marker_path.write_text(
            json.dumps(
                {
                    "root": _norm_root(self.root),
                    "touched_at": self.touched_at,
                    "stamp": newest_artifact_mtime(self.target_dir),
                }
            ),
            encoding="utf-8",
        )


def _release_exe_path(cargo_target_dir):
    name = "eqoa.exe" if sys.platform == "win32" else "eqoa"
    return Path(cargo_target_dir) / "release" / name


def build_steps(*, root, filters=None, extra_skips=None, release=False):
    root = Path(root)
    steps = []

    # 1. round
    if filters:
        filter_args = filters if isinstance(filters, list) else [filters]
        steps.append((
            "round",
            ["cargo", "test", "--locked", "-p", "eqoa", "--bin", "eqoa", "--", *filter_args, "--test-threads=1", "--nocapture"],
            True,
            True,
        ))

    # 2. addons
    steps.append((
        "addons",
        ["cargo", "test", "--locked", "-p", "eqoa", "--bin", "eqoa", "fork_ext::addons::", "--", "--test-threads=1", "--nocapture"],
        True,
        True,
    ))

    # 3. full
    skips = list(DEFAULT_FULL_SKIPS)
    if extra_skips:
        if isinstance(extra_skips, list):
            skips.extend(extra_skips)
        else:
            skips.append(extra_skips)
    skip_args = []
    for skip in skips:
        skip_args.extend(["--skip", skip])
    steps.append((
        "full",
        ["cargo", "test", "--locked", "-p", "eqoa", "--bin", "eqoa", "--", "--test-threads=1", "--nocapture", *skip_args],
        True,
        True,
    ))

    # 4. wasm
    steps.append((
        "wasm",
        ["cargo", "check", "--locked", "-p", "eqoa", "--target", "wasm32-unknown-unknown"],
        True,
        False,
    ))

    # 5. footprint
    base = read_reviewed_base(root)
    steps.append((
        "footprint",
        [sys.executable, "tools/check_fork_footprint.py", "--base", base],
        False,
        False,
    ))

    # 6. paths
    steps.append((
        "paths",
        [sys.executable, "-X", "utf8", "tools/check_no_local_paths.py"],
        False,
        False,
    ))

    # 7. player-release
    steps.append((
        "player-release",
        [sys.executable, "-X", "utf8", "tools/test_player_release.py"],
        False,
        False,
    ))

    # 8. release (only with --release)
    if release:
        steps.append((
            "release",
            ["cargo", "build", "--locked", "--release", "-p", "eqoa"],
            True,
            False,
        ))

    return steps


def run_gates(
    *,
    root=None,
    only=None,
    skip_steps=None,
    filters=None,
    extra_skips=None,
    release=False,
    dry_run=False,
    lock_wait=60,
    timeout=60,
    cargo_target_dir=None,
    lock_dir=None,
    branch=None,
    progress_interval=60,
):
    if root is None:
        root = Path.cwd()
    root = Path(root).resolve()

    if cargo_target_dir is None:
        cargo_target_dir = os.environ.get("CARGO_TARGET_DIR")
        if not cargo_target_dir:
            cargo_target_dir = str(Path.home() / "Paseo" / "targets" / "shared")
    cargo_target_dir = Path(cargo_target_dir)

    gates_dir = cargo_target_dir / "gates"
    if branch is None:
        branch = get_current_branch(root)
    sanitized_branch = sanitize_branch_name(branch)

    all_steps = build_steps(
        root=root,
        filters=filters,
        extra_skips=extra_skips,
        release=release,
    )

    if only:
        only_list = [s.strip() for s in only.split(",") if s.strip()]
        unknown = [s for s in only_list if s not in VALID_STEP_NAMES]
        if unknown:
            print(
                f"Error: Unknown step(s) in --only: {', '.join(unknown)}. "
                f"Valid step names: {', '.join(VALID_STEP_NAMES)}",
                file=sys.stderr,
            )
            return 2
        only_set = set(only_list)
        all_steps = [s for s in all_steps if s[0] in only_set]
        if not all_steps:
            print("Error: No runnable steps selected.", file=sys.stderr)
            return 2

    if skip_steps:
        skip_list = [s.strip() for s in skip_steps.split(",") if s.strip()]
        unknown = [s for s in skip_list if s not in VALID_STEP_NAMES]
        if unknown:
            print(
                f"Error: Unknown step(s) in --skip-step: {', '.join(unknown)}. "
                f"Valid step names: {', '.join(VALID_STEP_NAMES)}",
                file=sys.stderr,
            )
            return 2
        skip_set = set(skip_list)
        all_steps = [s for s in all_steps if s[0] not in skip_set]

    default_iso = root / "run" / "eqoa.iso"
    configured_iso = os.environ.get("EQOA_ISO")
    if not configured_iso and default_iso.is_file():
        configured_iso = str(default_iso)

    if dry_run:
        print("Dry run mode: commands and environment that would be run:")
        print(f"  CARGO_TARGET_DIR={cargo_target_dir}")
        print(f"  EQOA_CONFIG=<temporary directory per run>")
        if configured_iso:
            print(f"  EQOA_ISO={configured_iso}")
        for name, cmd, is_cargo, is_test in all_steps:
            env_note = " (with lock, RUSTFLAGS=+crt-static)" if name == "release" else (" (with lock)" if is_cargo else "")
            print(f"  [{name}]{env_note}: {' '.join(cmd)}")
        return 0

    gates_dir.mkdir(parents=True, exist_ok=True)
    step_results = []
    overall_success = True

    temp_config_dir = tempfile.mkdtemp(prefix="eqoa-gates-config-")
    try:
        for name, cmd, is_cargo, is_test in all_steps:
            log_path = gates_dir / f"{sanitized_branch}-{name}.log"
            step_env = os.environ.copy()
            step_env["CARGO_TARGET_DIR"] = str(cargo_target_dir)
            step_env["EQOA_CONFIG"] = str(temp_config_dir)
            if configured_iso:
                step_env["EQOA_ISO"] = configured_iso
            if name == "release":
                rustflags = step_env.get("RUSTFLAGS", "")
                step_env["RUSTFLAGS"] = f"{rustflags} -C target-feature=+crt-static".strip()

            print(f"==> Running step: {name}", flush=True)
            start_time = time.time()
            combined_output = ""
            step_passed = False
            tested_exes = []
            stale_error = None

            try:
                if is_cargo:
                    with CargoLock(lock_dir=lock_dir, wait_minutes=lock_wait):
                        guard = SharedTargetGuard(root, cargo_target_dir)
                        guard.begin()
                        returncode, combined_output = run_subprocess_tree(
                            cmd,
                            cwd=root,
                            env=step_env,
                            timeout_minutes=timeout,
                            step_name=name,
                            is_test=is_test,
                            progress_interval=progress_interval,
                        )
                        step_passed = (returncode == 0)
                        if step_passed and (is_test or name == "release"):
                            try:
                                if is_test:
                                    tested_exes = guard.verify(parse_test_exes(combined_output, root))
                                else:
                                    tested_exes = guard.verify([_release_exe_path(cargo_target_dir)])
                            except StaleBinaryError as exc:
                                step_passed = False
                                stale_error = str(exc)
                                combined_output += f"\nSTALE BINARY CHECK FAILED: {exc}\n"
                        guard.end()
                else:
                    returncode, combined_output = run_subprocess_tree(
                        cmd,
                        cwd=root,
                        env=step_env,
                        timeout_minutes=timeout,
                        step_name=name,
                        is_test=is_test,
                        progress_interval=progress_interval,
                    )
                    step_passed = (returncode == 0)
            except KeyboardInterrupt:
                raise
            except Exception as exc:
                combined_output = f"Execution error: {exc}\n"
                step_passed = False

            duration = time.time() - start_time
            log_path.write_text(combined_output, encoding="utf-8", errors="replace")

            if not step_passed:
                overall_success = False

            parsed_data = None
            if is_test:
                parsed_data = parse_cargo_test_output(combined_output)

            exe_path = None
            if name == "release" and step_passed:
                candidate = _release_exe_path(cargo_target_dir)
                if candidate.is_file():
                    exe_path = candidate
                else:
                    fallback = root / "target" / "release" / "eqoa.exe"
                    if fallback.is_file():
                        exe_path = fallback

            step_results.append({
                "name": name,
                "passed": step_passed,
                "duration": duration,
                "is_test": is_test,
                "parsed": parsed_data,
                "exe_path": exe_path,
                "tested_exes": tested_exes,
                "stale_error": stale_error,
                "log_path": log_path,
            })
    finally:
        shutil.rmtree(temp_config_dir, ignore_errors=True)

    summary_lines = []
    summary_lines.append(f"# Gates Summary for branch `{branch}`\n")
    for r in step_results:
        status_str = "PASS" if r["passed"] else "FAIL"
        dur_str = f"{r['duration']:.2f}s"
        summary_lines.append(f"- **{r['name']}**: {status_str} ({dur_str})")
        if r["exe_path"]:
            summary_lines.append(f"  - Executable: `{r['exe_path']}`")
        for exe, mtime in r["tested_exes"]:
            summary_lines.append(f"  - Tested executable: `{describe_exe(exe, mtime)}`")
        if r["stale_error"]:
            summary_lines.append(f"  - **STALE BINARY**: {r['stale_error']}")
        if r["is_test"] and r["parsed"]:
            p = r["parsed"]
            if p["found_test_results"]:
                summary_lines.append(
                    f"  - Results: {p['passed']} passed, {p['failed']} failed, {p['ignored']} ignored"
                )
            if p["failed_names"]:
                summary_lines.append(f"  - Failed tests ({len(p['failed_names'])}):")
                for fn in p["failed_names"]:
                    summary_lines.append(f"    - `{fn}`")
            if p["skip_count"] > 0:
                summary_lines.append(f"  - Skips ({p['skip_count']} lines, {len(p['distinct_skips'])} distinct):")
                for sk in p["distinct_skips"]:
                    summary_lines.append(f"    - {sk}")

    overall_status = "ALL GATES PASSED" if overall_success else "SOME GATES FAILED"
    summary_lines.append(f"\n**Status**: {overall_status}\n")

    summary_text = "\n".join(summary_lines)
    summary_path = gates_dir / f"{sanitized_branch}-summary.md"
    summary_path.write_text(summary_text, encoding="utf-8")

    print("\n" + summary_text)

    return 0 if overall_success else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root (default: current directory)")
    parser.add_argument("--only", help="comma-separated list of step names to run exclusively")
    parser.add_argument("--skip-step", help="comma-separated list of step names to skip")
    parser.add_argument("--filter", nargs="*", help="test filters for the round step")
    parser.add_argument("--extra-skip", action="append", help="extra tests to skip in full step")
    parser.add_argument("--release", action="store_true", help="run cargo build release step")
    parser.add_argument("--dry-run", action="store_true", help="print commands and environment without running")
    parser.add_argument("--lock-wait", type=int, default=60, help="lock wait timeout in minutes (default: 60)")
    parser.add_argument("--timeout", type=int, default=60, help="per-command timeout in minutes (default: 60)")
    args = parser.parse_args()

    return run_gates(
        root=args.root,
        only=args.only,
        skip_steps=args.skip_step,
        filters=args.filter,
        extra_skips=args.extra_skip,
        release=args.release,
        dry_run=args.dry_run,
        lock_wait=args.lock_wait,
        timeout=args.timeout,
    )


if __name__ == "__main__":
    sys.exit(main())
