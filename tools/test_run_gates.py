#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for run_gates.py."""
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest
from unittest import mock

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import run_gates

SAMPLE_CARGO_TEST_OUTPUT = """
running 3 tests
test fork_ext::addons::tests::test_one ... ok
test fork_ext::addons::tests::test_two ... FAILED
test fork_ext::addons::tests::test_three ... ok

failures:

---- fork_ext::addons::tests::test_two stdout ----
thread 'fork_ext::addons::tests::test_two' panicked at 'assertion failed'
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    fork_ext::addons::tests::test_two

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

running 2 tests
test oracle_check_one ... ok [SKIP: oracle unavailable]
test oracle_check_two ... ok [SKIP: disc missing]
test oracle_check_one ... ok [SKIP: oracle unavailable]

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.05s
"""


class TestRunGates(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory(prefix="test-run-gates-")
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name) / "repo"
        self.root.mkdir()
        docs = self.root / "docs"
        docs.mkdir()
        (docs / "fork-footprint.json").write_text(
            json.dumps({"reviewed_base": "1234567890abcdef1234567890abcdef12345678"}),
            encoding="utf-8",
        )

    def test_parse_cargo_test_output_and_skips(self):
        parsed = run_gates.parse_cargo_test_output(SAMPLE_CARGO_TEST_OUTPUT)
        self.assertTrue(parsed["found_test_results"])
        self.assertEqual(parsed["passed"], 4)
        self.assertEqual(parsed["failed"], 1)
        self.assertEqual(parsed["ignored"], 1)
        self.assertIn("fork_ext::addons::tests::test_two", parsed["failed_names"])
        self.assertEqual(parsed["skip_count"], 3)
        self.assertEqual(len(parsed["distinct_skips"]), 2)
        self.assertIn("test oracle_check_one ... ok [SKIP: oracle unavailable]", parsed["distinct_skips"])
        self.assertIn("test oracle_check_two ... ok [SKIP: disc missing]", parsed["distinct_skips"])

    def test_read_reviewed_base(self):
        base = run_gates.read_reviewed_base(self.root)
        self.assertEqual(base, "1234567890abcdef1234567890abcdef12345678")

    def test_lock_acquire_and_release(self):
        lock_dir = Path(self.temp_dir.name) / "test.lock"
        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=1, poll_interval=0.1)

        self.assertFalse(lock_dir.exists())
        with lock:
            self.assertTrue(lock_dir.exists())
            self.assertTrue(lock.acquired)
        self.assertFalse(lock_dir.exists())
        self.assertFalse(lock.acquired)

    def test_lock_released_on_exception(self):
        lock_dir = Path(self.temp_dir.name) / "test_exc.lock"
        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=1, poll_interval=0.1)

        try:
            with lock:
                self.assertTrue(lock_dir.exists())
                raise RuntimeError("Simulated command failure")
        except RuntimeError:
            pass

        self.assertFalse(lock_dir.exists())
        self.assertFalse(lock.acquired)

    def test_lock_wait_when_exists(self):
        lock_dir = Path(self.temp_dir.name) / "test_wait.lock"
        lock_dir.mkdir(parents=True, exist_ok=True)

        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=0.01, poll_interval=0.05)
        with self.assertRaises(TimeoutError):
            lock.acquire()

        lock_dir.rmdir()
        lock2 = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=1, poll_interval=0.05)
        with lock2:
            self.assertTrue(lock_dir.exists())
        self.assertFalse(lock_dir.exists())

    def test_lock_clears_a_lock_whose_holder_died(self):
        # A killed gate run leaves its folder and token behind; the next run must take over at once.
        dead = subprocess.Popen([sys.executable, "-c", "pass"])
        dead.wait()
        lock_dir = Path(self.temp_dir.name) / "stale.lock"
        lock_dir.mkdir(parents=True, exist_ok=True)
        (lock_dir / f"pid-{dead.pid}-1.token").touch()
        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=0.01, poll_interval=0.05)
        with lock:
            self.assertTrue(lock.acquired)
            self.assertEqual([p.name for p in lock_dir.iterdir()], [lock.owner_token])
        self.assertFalse(lock_dir.exists())

    def test_lock_reclaims_old_empty_directory_only_when_build_is_idle(self):
        lock_dir = Path(self.temp_dir.name) / "empty.lock"
        lock_dir.mkdir()
        lock = run_gates.CargoLock(lock_dir=lock_dir)
        old = time.time() - 600
        os.utime(lock_dir, (old, old))
        with mock.patch.object(run_gates, "cargo_process_running", return_value=True, create=True):
            self.assertFalse(lock._clear_if_holder_dead())
        self.assertTrue(lock_dir.is_dir())
        with mock.patch.object(run_gates, "cargo_process_running", return_value=False, create=True):
            self.assertTrue(lock._clear_if_holder_dead())
        self.assertFalse(lock_dir.exists())

    def test_lock_keeps_a_live_holder_and_a_tokenless_lock(self):
        for name, token in (("live.lock", True), ("plain.lock", False)):
            lock_dir = Path(self.temp_dir.name) / name
            lock_dir.mkdir(parents=True, exist_ok=True)
            if token:
                # a running process other than this one: the parent of this test process
                (lock_dir / f"pid-{os.getppid()}-1.token").touch()
            lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=0.005, poll_interval=0.05)
            with self.assertRaises(TimeoutError):
                lock.acquire()
            self.assertTrue(lock_dir.exists())

    def test_process_alive(self):
        self.assertTrue(run_gates.process_alive(os.getpid()))
        dead = subprocess.Popen([sys.executable, "-c", "pass"])
        dead.wait()
        self.assertFalse(run_gates.process_alive(dead.pid))

    def test_lock_never_removes_a_foreign_lock_on_mkdir_error(self):
        # Another agent's lock (a plain empty folder) exists; mkdir reports access denied, as Windows
        # does while a holder removes it. The foreign lock must survive and acquire must time out.
        lock_dir = Path(self.temp_dir.name) / "foreign.lock"
        lock_dir.mkdir(parents=True, exist_ok=True)
        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=0.005, poll_interval=0.05)
        with mock.patch.object(run_gates.os, "mkdir", side_effect=PermissionError("denied")):
            with self.assertRaises(TimeoutError):
                lock.acquire()
        self.assertTrue(lock_dir.exists())
        self.assertFalse(lock.acquired)

    def test_step_selection_only_and_skip(self):
        steps = run_gates.build_steps(root=self.root, filters="my_filter", release=True)
        step_names = [s[0] for s in steps]
        self.assertIn("round", step_names)
        self.assertIn("addons", step_names)
        self.assertIn("full", step_names)
        self.assertIn("wasm", step_names)
        self.assertIn("footprint", step_names)
        self.assertIn("paths", step_names)
        self.assertIn("player-release", step_names)
        self.assertIn("release", step_names)

        steps_no_filter = run_gates.build_steps(root=self.root, filters=None, release=False)
        names_no_filter = [s[0] for s in steps_no_filter]
        self.assertNotIn("round", names_no_filter)
        self.assertNotIn("release", names_no_filter)

    def test_unknown_step_name_returns_error_exit_code_2(self):
        target_dir = Path(self.temp_dir.name) / "target"
        stderr_capture = io.StringIO()
        with mock.patch("sys.stderr", stderr_capture):
            exit_code = run_gates.run_gates(
                root=self.root,
                only="nonexistent_step",
                cargo_target_dir=target_dir,
                branch="test-feat",
            )
        self.assertEqual(exit_code, 2)
        err_msg = stderr_capture.getvalue()
        self.assertIn("Unknown step(s) in --only: nonexistent_step", err_msg)
        for valid in run_gates.VALID_STEP_NAMES:
            self.assertIn(valid, err_msg)

        # Also test --skip-step with unknown step
        stderr_capture = io.StringIO()
        with mock.patch("sys.stderr", stderr_capture):
            exit_code = run_gates.run_gates(
                root=self.root,
                skip_steps="typo_step",
                cargo_target_dir=target_dir,
                branch="test-feat",
            )
        self.assertEqual(exit_code, 2)
        self.assertIn("Unknown step(s) in --skip-step: typo_step", stderr_capture.getvalue())

    def test_multiple_filter_words_dry_run(self):
        target_dir = Path(self.temp_dir.name) / "target"
        output_capture = io.StringIO()
        with mock.patch("sys.stdout", output_capture):
            exit_code = run_gates.run_gates(
                root=self.root,
                filters=["filter_one", "filter_two"],
                only="round",
                dry_run=True,
                cargo_target_dir=target_dir,
                branch="test-feat",
            )
        self.assertEqual(exit_code, 0)
        output = output_capture.getvalue()
        self.assertIn("-- filter_one filter_two --test-threads=1", output)

    @mock.patch("run_gates.run_subprocess_tree")
    def test_dry_run_executes_nothing(self, mock_run_tree):
        target_dir = Path(self.temp_dir.name) / "target"
        output_capture = io.StringIO()
        with mock.patch("sys.stdout", output_capture):
            exit_code = run_gates.run_gates(
                root=self.root,
                dry_run=True,
                cargo_target_dir=target_dir,
                branch="test-feat",
            )
        self.assertEqual(exit_code, 0)
        mock_run_tree.assert_not_called()
        self.assertIn("Dry run mode", output_capture.getvalue())

    @mock.patch("run_gates.kill_process_tree")
    @mock.patch("subprocess.Popen")
    def test_timeout_kills_process_tree_and_decodes_bytes(self, mock_popen, mock_kill):
        mock_proc = mock.MagicMock()
        mock_proc.pid = 99999
        mock_proc.returncode = 0
        # First communicate raises TimeoutExpired with bytes in stdout
        mock_proc.communicate.side_effect = [
            subprocess.TimeoutExpired(cmd="cargo test", timeout=60, output=b"some bytes output before timeout"),
            (b"final drained bytes", None),
        ]
        mock_popen.return_value = mock_proc

        ret, text = run_gates.run_subprocess_tree(
            ["cargo", "test"],
            cwd=self.root,
            env={},
            timeout_minutes=1,
        )

        mock_kill.assert_called_once_with(99999)
        self.assertEqual(ret, -1)
        self.assertIn("final drained bytes", text)
        self.assertIn("Command timed out after 1 minutes", text)

    @mock.patch("run_gates.kill_process_tree")
    @mock.patch("subprocess.Popen")
    def test_ctrl_c_kills_process_tree_and_raises(self, mock_popen, mock_kill):
        mock_proc = mock.MagicMock()
        mock_proc.pid = 88888
        mock_proc.communicate.side_effect = KeyboardInterrupt()
        mock_popen.return_value = mock_proc

        with self.assertRaises(KeyboardInterrupt):
            run_gates.run_subprocess_tree(
                ["cargo", "test"],
                cwd=self.root,
                env={},
                timeout_minutes=1,
            )

        mock_kill.assert_called_once_with(88888)

    @mock.patch("run_gates.run_subprocess_tree")
    def test_timeout_step_writes_fail_log_and_subsequent_steps_run(self, mock_run_tree):
        target_dir = Path(self.temp_dir.name) / "target"
        lock_dir = Path(self.temp_dir.name) / "test_flow.lock"

        # Step 1 (footprint) times out, Step 2 (paths) passes
        mock_run_tree.side_effect = [
            (-1, "Step timed out\nCommand timed out after 60 minutes\n"),
            (0, "All paths clean\n"),
        ]

        exit_code = run_gates.run_gates(
            root=self.root,
            only="footprint,paths",
            cargo_target_dir=target_dir,
            lock_dir=lock_dir,
            branch="test-feat",
        )

        self.assertEqual(exit_code, 1)  # failed because step 1 failed
        self.assertEqual(mock_run_tree.call_count, 2)  # step 2 still ran!

        footprint_log = target_dir / "gates" / "test-feat-footprint.log"
        self.assertTrue(footprint_log.is_file())
        self.assertIn("Command timed out after 60 minutes", footprint_log.read_text("utf-8"))

        summary_file = target_dir / "gates" / "test-feat-summary.md"
        self.assertTrue(summary_file.is_file())
        summary_content = summary_file.read_text("utf-8")
        self.assertIn("- **footprint**: FAIL", summary_content)
        self.assertIn("- **paths**: PASS", summary_content)
        self.assertIn("SOME GATES FAILED", summary_content)

    def test_progress_lines_and_test_counts_for_slow_step(self):
        script = (
            "import time\n"
            "print('test alpha ... ok', flush=True)\n"
            "time.sleep(0.12)\n"
            "print('test beta ... FAILED', flush=True)\n"
            "time.sleep(0.12)\n"
            "print('test gamma ... ignored', flush=True)\n"
            "time.sleep(0.12)\n"
            "print('test result: FAILED. 1 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out', flush=True)\n"
        )
        stdout_capture = io.StringIO()
        with mock.patch("sys.stdout", stdout_capture):
            ret, text = run_gates.run_subprocess_tree(
                [sys.executable, "-u", "-c", script],
                cwd=self.root,
                env=os.environ.copy(),
                timeout_minutes=1,
                step_name="full",
                is_test=True,
                progress_interval=0.05,
            )

        self.assertEqual(ret, 0)
        self.assertIn("test alpha ... ok", text)
        self.assertIn("test beta ... FAILED", text)
        self.assertIn("test gamma ... ignored", text)
        self.assertIn("test result: FAILED.", text)

        progress_lines = [
            line for line in stdout_capture.getvalue().splitlines()
            if "[full]" in line and "m elapsed" in line
        ]
        self.assertGreaterEqual(len(progress_lines), 2)
        counts = []
        for line in progress_lines:
            self.assertIn("tests)", line)
            count_str = line.rsplit("(", 1)[1].split(" tests)", 1)[0]
            counts.append(int(count_str))
        self.assertEqual(counts, sorted(counts))
        self.assertLessEqual(max(counts), 3)
        self.assertGreaterEqual(max(counts), 1)

    def test_progress_lines_for_non_test_step(self):
        script = "import time\nprint('Compiling eqoa', flush=True)\ntime.sleep(0.15)\n"
        stdout_capture = io.StringIO()
        with mock.patch("sys.stdout", stdout_capture):
            ret, text = run_gates.run_subprocess_tree(
                [sys.executable, "-u", "-c", script],
                cwd=self.root,
                env=os.environ.copy(),
                timeout_minutes=1,
                step_name="wasm",
                is_test=False,
                progress_interval=0.05,
            )

        self.assertEqual(ret, 0)
        self.assertIn("Compiling eqoa", text)
        progress_lines = [
            line for line in stdout_capture.getvalue().splitlines()
            if "[wasm]" in line and "m elapsed" in line
        ]
        self.assertGreaterEqual(len(progress_lines), 1)
        for line in progress_lines:
            self.assertNotIn("tests", line)

    def test_lock_wait_prints_holder_pid(self):
        lock_dir = Path(self.temp_dir.name) / "holder.lock"
        lock_dir.mkdir(parents=True, exist_ok=True)
        holder_pid = os.getppid()
        (lock_dir / f"pid-{holder_pid}-1.token").touch()
        lock = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=0.005, poll_interval=0.05)

        stdout_capture = io.StringIO()
        with mock.patch("sys.stdout", stdout_capture):
            with self.assertRaises(TimeoutError):
                lock.acquire()

        output = stdout_capture.getvalue()
        self.assertIn("Waiting for cargo lock", output)
        self.assertIn(f"held by pid {holder_pid}", output)

    def test_streaming_timeout_after_child_closes_stdout_then_hangs(self):
        script = (
            "import os, sys, time\n"
            "print('before close', flush=True)\n"
            "sys.stdout.flush()\n"
            "sys.stderr.flush()\n"
            "os.close(1)\n"
            "os.close(2)\n"
            "time.sleep(30)\n"
        )
        ret, text = run_gates.run_subprocess_tree(
            [sys.executable, "-u", "-c", script],
            cwd=self.root,
            env=os.environ.copy(),
            timeout_minutes=0.005,
            step_name="full",
            is_test=False,
            progress_interval=0.05,
        )
        self.assertEqual(ret, -1)
        self.assertIn("before close", text)
        self.assertIn("Command timed out after 0.005 minutes", text)

    @mock.patch("run_gates.kill_process_tree")
    @mock.patch("subprocess.Popen")
    def test_streaming_reader_error_fails_step_visibly(self, mock_popen, mock_kill):
        mock_proc = mock.MagicMock()
        mock_proc.pid = 77777
        mock_proc.returncode = 0
        mock_proc.stdout.readline.side_effect = OSError("simulated read failure")
        mock_popen.return_value = mock_proc

        stderr_capture = io.StringIO()
        with mock.patch("sys.stderr", stderr_capture):
            ret, text = run_gates.run_subprocess_tree(
                ["cargo", "test"],
                cwd=self.root,
                env={},
                timeout_minutes=1,
                step_name="full",
                is_test=True,
                progress_interval=0.05,
            )

        mock_kill.assert_called_once_with(77777)
        self.assertEqual(ret, -1)
        self.assertIn("Stream reader error for full: simulated read failure", text)
        self.assertIn("Stream reader error for full: simulated read failure", stderr_capture.getvalue())


class TestSharedTargetGuard(unittest.TestCase):
    """The shared CARGO_TARGET_DIR stale-binary guard (no cargo is run; timestamps are set by hand)."""

    HOUR = 3600

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory(prefix="test-run-gates-guard-")
        self.addCleanup(self.temp_dir.cleanup)
        base = Path(self.temp_dir.name)
        self.root = base / "wt-a"
        self.other_root = base / "wt-b"
        self.target = base / "target"
        self.past = time.time() - 10 * self.HOUR
        (self.root / "docs").mkdir(parents=True)
        (self.root / "docs" / "fork-footprint.json").write_text(
            json.dumps({"reviewed_base": "1234567890abcdef1234567890abcdef12345678"}), encoding="utf-8"
        )
        self.sources = []
        for crate, names in {"eqoa": ["src/main.rs", "build.rs", "src/other.rs"], "vi": ["src/lib.rs"]}.items():
            for name in names:
                path = self.root / "crates" / crate / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// x\n", encoding="utf-8")
                os.utime(path, (self.past, self.past))
                self.sources.append(path)
        self.deps = self.target / "debug" / "deps"
        self.deps.mkdir(parents=True)

    def _exe(self, mtime, name="eqoa-abc.exe"):
        exe = self.deps / name
        exe.write_bytes(b"MZ")
        os.utime(exe, (mtime, mtime))
        return exe

    def _dep_info(self, mtime):
        d = self.deps / "eqoa-abc.d"
        d.write_text("x", encoding="utf-8")
        os.utime(d, (mtime, mtime))

    def _gate(self, fake_tree, *, branch, only="addons", release=False):
        with mock.patch("run_gates.run_subprocess_tree", side_effect=fake_tree), mock.patch("sys.stdout", io.StringIO()):
            code = run_gates.run_gates(
                root=self.root,
                only=only,
                release=release,
                cargo_target_dir=self.target,
                lock_dir=Path(self.temp_dir.name) / "cargo.lock",
                branch=branch,
            )
        gates = self.target / "gates"
        summary = (gates / f"{branch}-summary.md").read_text("utf-8")
        log = (gates / f"{branch}-{only}.log").read_text("utf-8")
        return code, summary, log

    def test_touch_marks_only_workspace_root_files_new(self):
        now, files = run_gates.touch_workspace_sources(self.root)
        self.assertEqual(sorted(f.name for f in files), ["build.rs", "lib.rs", "main.rs"])
        for path in self.sources:
            if path.name == "other.rs":
                self.assertLess(path.stat().st_mtime, self.past + 1)
            else:
                self.assertAlmostEqual(path.stat().st_mtime, now, delta=1.0)

    def test_needs_touch_rules(self):
        marker = {"root": run_gates._norm_root(self.root), "touched_at": 5.0, "stamp": 100.0}
        self.assertTrue(run_gates.guard_needs_touch(None, self.root, 100.0))
        self.assertFalse(run_gates.guard_needs_touch(marker, self.root, 100.0))
        self.assertTrue(run_gates.guard_needs_touch(marker, self.other_root, 100.0))  # another worktree built last
        self.assertTrue(run_gates.guard_needs_touch(marker, self.root, 250.0))  # someone built since
        self.assertTrue(run_gates.guard_needs_touch({"root": marker["root"]}, self.root, 100.0))  # damaged marker

    def test_newest_artifact_mtime(self):
        self.assertEqual(run_gates.newest_artifact_mtime(self.target), 0.0)
        self._dep_info(self.past)
        release = self.target / "release" / "deps"
        release.mkdir(parents=True)
        newer = release / "eqoa-def.d"
        newer.write_text("x", encoding="utf-8")
        os.utime(newer, (self.past + 50, self.past + 50))
        self.assertAlmostEqual(run_gates.newest_artifact_mtime(self.target), self.past + 50, delta=0.01)

    def test_parse_test_exes(self):
        out = (
            "   Compiling eqoa v0.1.0\n"
            "    Finished `test` profile [unoptimized] target(s) in 0.87s\n"
            "     Running unittests src/main.rs (C:\\t\\debug\\deps\\eqoa-abc.exe)\n"
            "     Running unittests src/main.rs (C:\\t\\debug\\deps\\eqoa-abc.exe)\n"
            "     Running unittests src/lib.rs (target\\debug\\deps\\vi-1.exe)\n"
        )
        self.assertEqual(
            run_gates.parse_test_exes(out, self.root),
            [Path("C:\\t\\debug\\deps\\eqoa-abc.exe"), self.root / "target" / "debug" / "deps" / "vi-1.exe"],
        )
        self.assertEqual(run_gates.parse_test_exes("nothing here\n"), [])

    def test_verify_accepts_rebuilt_and_rejects_stale_or_unknown_exe(self):
        guard = run_gates.SharedTargetGuard(self.root, self.target)
        touched = guard.begin()
        fresh = self._exe(touched + 5)
        stale = self._exe(touched - self.HOUR, name="eqoa-old.exe")
        ((exe, mtime),) = guard.verify([fresh])
        self.assertEqual(exe, fresh)
        self.assertAlmostEqual(mtime, touched + 5, delta=0.01)
        with self.assertRaisesRegex(run_gates.StaleBinaryError, "not rebuilt"):
            guard.verify([fresh, stale])
        with self.assertRaisesRegex(run_gates.StaleBinaryError, "could not identify"):
            guard.verify([])
        with self.assertRaisesRegex(run_gates.StaleBinaryError, "missing"):
            guard.verify([self.deps / "gone.exe"])

    def test_begin_skips_touch_only_while_this_worktree_built_last(self):
        guard = run_gates.SharedTargetGuard(self.root, self.target)
        first = guard.begin()
        self.assertTrue(guard.touched_now)
        self._dep_info(first + 2)  # this worktree's build
        guard.end()

        again = run_gates.SharedTargetGuard(self.root, self.target)
        self.assertEqual(again.begin(), first)  # nothing built since: no re-touch, no needless rebuild
        self.assertFalse(again.touched_now)

        time.sleep(0.05)  # coarse Windows clock: keep touches on distinct stamps
        self._dep_info(time.time() + 60)  # another worktree builds
        third = run_gates.SharedTargetGuard(self.root, self.target)
        self.assertGreater(third.begin(), first)
        self.assertTrue(third.touched_now)

        # The owner package builder forces a touch even when the marker says this worktree built last.
        third.end()
        time.sleep(0.05)  # Windows runners tick time.time() at ~15 ms; two touches can share a stamp
        forced = run_gates.SharedTargetGuard(self.root, self.target)
        self.assertGreater(forced.begin(force=True), third.touched_at)
        self.assertTrue(forced.touched_now)

    def _addons_step(self, exe_offset_from_touch, branch):
        """Run the addons step with a fake cargo reporting an exe whose mtime is touch time + offset."""
        exe = self.deps / "eqoa-run.exe"
        exe.write_bytes(b"MZ")

        def fake_tree(cmd, cwd, env, timeout_minutes, **kwargs):
            touched = max(p.stat().st_mtime for p in self.sources)  # the guard has touched by now
            mtime = touched + exe_offset_from_touch
            os.utime(exe, (mtime, mtime))
            return 0, (
                "    Finished `test` profile target(s) in 0.87s\n"
                f"     Running unittests src/main.rs ({exe})\n"
                "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n"
            )

        return (*self._gate(fake_tree, branch=branch), exe)

    def test_gate_passes_and_records_exe_when_rebuilt_after_touch(self):
        code, summary, log, exe = self._addons_step(5.0, "fresh")
        self.assertEqual(code, 0)
        self.assertIn("- **addons**: PASS", summary)
        self.assertIn(f"Tested executable: `{exe}", summary)
        self.assertIn("(modified ", summary)
        self.assertNotIn("STALE BINARY", summary)

    def test_gate_fails_loudly_when_cargo_reused_another_worktrees_exe(self):
        # The exe is an hour older than this worktree's touch: another worktree built it.
        code, summary, log, exe = self._addons_step(-self.HOUR, "stale")
        self.assertEqual(code, 1)
        self.assertIn("- **addons**: FAIL", summary)
        self.assertIn("**STALE BINARY**", summary)
        self.assertIn("SOME GATES FAILED", summary)
        self.assertIn("STALE BINARY CHECK FAILED", log)

    def test_gate_without_a_reported_exe_fails(self):
        def fake_tree(cmd, cwd, env, timeout_minutes, **kwargs):
            return 0, "test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n"

        code, summary, log = self._gate(fake_tree, branch="noexe")
        self.assertEqual(code, 1)
        self.assertIn("could not identify", summary)

    def test_release_step_checks_exe_is_rebuilt(self):
        release_exe = run_gates._release_exe_path(self.target)
        release_exe.parent.mkdir(parents=True)
        release_exe.write_bytes(b"MZ")
        os.utime(release_exe, (self.past, self.past))  # left by another worktree

        def fake_tree(cmd, cwd, env, timeout_minutes, **kwargs):
            return 0, "    Finished `release` profile [optimized] target(s) in 0.9s\n"

        code, summary, log = self._gate(fake_tree, branch="rel", only="release", release=True)
        self.assertEqual(code, 1)
        self.assertIn("**STALE BINARY**", summary)


if __name__ == "__main__":
    unittest.main()
