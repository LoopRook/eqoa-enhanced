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


if __name__ == "__main__":
    unittest.main()
