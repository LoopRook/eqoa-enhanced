#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for run_gates.py."""
import io
import json
import os
from pathlib import Path
import shutil
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
        # Should timeout quickly because lock_dir already exists
        with self.assertRaises(TimeoutError):
            lock.acquire()

        # If existing lock is removed externally, acquire succeeds
        lock_dir.rmdir()
        lock2 = run_gates.CargoLock(lock_dir=lock_dir, wait_minutes=1, poll_interval=0.05)
        with lock2:
            self.assertTrue(lock_dir.exists())
        self.assertFalse(lock_dir.exists())

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

        # round without filter should be omitted
        steps_no_filter = run_gates.build_steps(root=self.root, filters=None, release=False)
        names_no_filter = [s[0] for s in steps_no_filter]
        self.assertNotIn("round", names_no_filter)
        self.assertNotIn("release", names_no_filter)

    @mock.patch("subprocess.run")
    def test_dry_run_executes_nothing(self, mock_run):
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
        mock_run.assert_not_called()
        self.assertIn("Dry run mode", output_capture.getvalue())

    @mock.patch("subprocess.run")
    def test_run_gates_flow_with_mocked_subprocess(self, mock_run):
        target_dir = Path(self.temp_dir.name) / "target"
        lock_dir = Path(self.temp_dir.name) / "test_flow.lock"

        def fake_subprocess_run(cmd, **kwargs):
            mock_proc = mock.MagicMock()
            mock_proc.returncode = 0
            if "cargo" in cmd and "test" in cmd:
                mock_proc.stdout = SAMPLE_CARGO_TEST_OUTPUT
            else:
                mock_proc.stdout = "OK\n"
            return mock_proc

        mock_run.side_effect = fake_subprocess_run

        exit_code = run_gates.run_gates(
            root=self.root,
            only="footprint,paths",
            cargo_target_dir=target_dir,
            lock_dir=lock_dir,
            branch="test-feat",
        )
        self.assertEqual(exit_code, 0)
        self.assertEqual(mock_run.call_count, 2)
        summary_file = next((target_dir / "gates").glob("*-summary.md"), None)
        self.assertIsNotNone(summary_file)
        content = summary_file.read_text(encoding="utf-8")
        self.assertIn("ALL GATES PASSED", content)


if __name__ == "__main__":
    unittest.main()
