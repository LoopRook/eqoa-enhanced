# SPDX-License-Identifier: AGPL-3.0-or-later
"""ci_gates.py: argument handling and the ref-refusal rule. No network, no git writes."""
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import ci_gates
from ci_gates import RefusedRef, ci_branch, delete_command, push_command, require_ci_ref

BAD_NAMES = [
    "", "   ", "-x", "/abs", "a//b", "a/../b", "a..b", "a:b", "a b", "a*b", "a?b", "a[b", "a^b", "a~1", "a\\b",
    "a@{1}", "feat/", "feat.", "x.lock", "a/.hidden", "a/b.lock/c", "refs/tags/v1", "HEAD:refs/heads/develop",
    "+develop", "ci/", "refs/heads/",
]


class RefusalTests(unittest.TestCase):
    def test_plain_names_map_under_ci(self):
        self.assertEqual(ci_branch("feat/ci-branch-gates"), "ci/feat/ci-branch-gates")
        self.assertEqual(ci_branch("ci/feat/x"), "ci/feat/x")
        self.assertEqual(ci_branch("refs/heads/fix/a.b_c"), "ci/fix/a.b_c")

    def test_every_malformed_or_foreign_name_is_refused(self):
        for name in BAD_NAMES:
            with self.subTest(name=name):
                with self.assertRaises(RefusedRef):
                    ci_branch(name)
                with self.assertRaises(RefusedRef):
                    push_command(name)
                with self.assertRaises(RefusedRef):
                    delete_command(name)

    def test_push_writes_only_a_ci_ref_and_forces_only_that(self):
        command = push_command("feat/x")
        self.assertEqual(command[-1], "refs/heads/feat/x:refs/heads/ci/feat/x")
        self.assertEqual(command.count("--force"), 1)
        self.assertEqual(command[command.index("--force") + 1], ci_gates.REMOTE)
        destinations = [part.split(":")[1] for part in command if ":" in part and part.startswith("refs/")]
        self.assertEqual(destinations, ["refs/heads/ci/feat/x"])

    def test_a_branch_named_like_a_protected_one_still_lands_under_ci(self):
        for name in ("develop", "enhanced", "master"):
            self.assertEqual(push_command(name)[-1], f"refs/heads/{name}:refs/heads/ci/{name}")
            self.assertEqual(delete_command(name)[-1], f"refs/heads/ci/{name}")

    def test_the_last_guard_rejects_non_ci_refs(self):
        for ref in ("refs/heads/develop", "refs/heads/enhanced", "refs/heads/master", "refs/tags/v1",
                    "refs/heads/ci/", "refs/heads/cix/y", "ci/feat", "refs/heads/ci/a:b", "develop"):
            with self.subTest(ref=ref):
                with self.assertRaises(RefusedRef):
                    require_ci_ref(ref)
        self.assertEqual(require_ci_ref("refs/heads/ci/feat"), "refs/heads/ci/feat")

    def test_delete_never_uses_force_or_a_wildcard(self):
        command = delete_command("feat/x")
        self.assertNotIn("--force", command)
        self.assertNotIn("-f", command)
        self.assertEqual(command[-2:], ["--delete", "refs/heads/ci/feat/x"])
        self.assertFalse(any("*" in part for part in command[4:]))

    def test_negative_control_a_weakened_guard_would_let_develop_through(self):
        # If the guard only checked "starts with refs/heads/", develop would pass: show it would.
        weak = lambda ref: ref.startswith("refs/heads/")
        self.assertTrue(weak("refs/heads/develop"))
        with self.assertRaises(RefusedRef):
            require_ci_ref("refs/heads/develop")


class CommandLineTests(unittest.TestCase):
    def test_refused_branch_exits_2_before_any_subprocess(self):
        with mock.patch("ci_gates.subprocess.run") as run:
            for argv in (["push", "a:b"], ["cleanup", "refs/tags/v1"], ["status", "x y"]):
                with self.subTest(argv=argv):
                    self.assertEqual(ci_gates.main(argv), 2)
            run.assert_not_called()

    def test_cleanup_needs_exactly_one_of_branch_or_merged(self):
        for argv in (["cleanup"], ["cleanup", "x", "--merged"]):
            with self.subTest(argv=argv):
                with self.assertRaises(SystemExit):
                    ci_gates.main(argv)

    def test_unknown_command_and_missing_branch_fail(self):
        for argv in ([], ["bogus"], ["push"], ["status"]):
            with self.subTest(argv=argv), self.assertRaises(SystemExit):
                ci_gates.main(argv)

    def test_push_issues_exactly_one_forced_write_to_the_ci_ref(self):
        calls = []

        def fake(cmd, check=True):
            calls.append(cmd)
            return mock.Mock(stdout="a" * 40 + "\n", stderr="", returncode=0)

        with mock.patch("ci_gates.run", fake), mock.patch("ci_gates.find_run", return_value=None):
            self.assertEqual(ci_gates.main(["push", "feat/x"]), 0)
        writes = [c for c in calls if "push" in c]
        self.assertEqual(len(writes), 1)
        self.assertEqual(writes[0][-1], "refs/heads/feat/x:refs/heads/ci/feat/x")

    def test_cleanup_merged_deletes_only_merged_ci_branches(self):
        merged_sha, open_sha = "1" * 40, "2" * 40
        calls = []

        def fake(cmd, check=True):
            calls.append(cmd)
            if "ls-remote" in cmd:
                out = f"{merged_sha}\trefs/heads/ci/feat/done\n{open_sha}\trefs/heads/ci/feat/open\n"
                return mock.Mock(stdout=out, stderr="", returncode=0)
            if "rev-parse" in cmd:
                return mock.Mock(stdout="d" * 40 + "\n", stderr="", returncode=0)
            if "merge-base" in cmd:
                return mock.Mock(stdout="", stderr="", returncode=0 if cmd[-2] == merged_sha else 1)
            return mock.Mock(stdout="", stderr="", returncode=0)

        with mock.patch("ci_gates.run", fake):
            self.assertEqual(ci_gates.main(["cleanup", "--merged"]), 0)
        deletes = [c for c in calls if "--delete" in c]
        self.assertEqual([c[-1] for c in deletes], ["refs/heads/ci/feat/done"])

    def test_cleanup_dry_run_deletes_nothing(self):
        calls = []

        def fake(cmd, check=True):
            calls.append(cmd)
            return mock.Mock(stdout="", stderr="", returncode=0)

        with mock.patch("ci_gates.run", fake):
            self.assertEqual(ci_gates.main(["cleanup", "feat/x", "--dry-run"]), 0)
        self.assertEqual(calls, [])


class RunFilterTests(unittest.TestCase):
    def test_only_this_workflow_counts_under_either_naming(self):
        runs = [
            {"workflowName": ".github/workflows/branch-gates.yml", "name": "Branch gates", "id": 1},
            {"workflowName": ".github/workflows/branch-gates.yml", "name": ".github/workflows/branch-gates.yml", "id": 2},
            {"workflowName": "Branch gates", "name": "Branch gates", "id": 3},
            {"workflowName": "Nightly build", "name": "Nightly build", "id": 4},
            {"workflowName": ".github/workflows/enhanced-dev.yml", "name": "x", "id": 5},
        ]
        self.assertEqual([r["id"] for r in ci_gates.only_gate_runs(runs)], [1, 2, 3])


class SummaryTests(unittest.TestCase):
    CARGO = (
        "test a ... ok\ntest b ... FAILED\nSKIP: no disc image at run/eqoa.iso\nSKIP: no disc on hand\n"
        "failures:\n    b\n\ntest result: FAILED. 1 passed; 1 failed; 3 ignored; 0 measured; 0 filtered out\n"
        "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
    )

    def test_cargo_log_counts_failures_and_skips(self):
        parsed = ci_gates.parse_gate_log(self.CARGO)
        self.assertEqual((parsed["passed"], parsed["failed"], parsed["ignored"]), (5, 1, 3))
        self.assertEqual(parsed["skips"], 2)
        self.assertEqual(parsed["failing"], ["b"])

    def test_unittest_log(self):
        log = "FAIL: test_x (m.C.test_x)\nERROR: test_y (m.C.test_y)\nRan 10 tests in 1s\n\nFAILED (failures=1, errors=1, skipped=2)\n"
        parsed = ci_gates.parse_gate_log(log)
        self.assertEqual((parsed["passed"], parsed["failed"], parsed["ignored"]), (6, 2, 2))
        self.assertEqual(parsed["failing"], ["test_x (m.C.test_x)", "test_y (m.C.test_y)"])

    def test_empty_log_has_no_counts(self):
        parsed = ci_gates.parse_gate_log("")
        self.assertIsNone(parsed["passed"])
        self.assertEqual(parsed["failing"], [])

    def test_summary_fails_when_any_gate_outcome_is_not_success(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "full.log").write_text(self.CARGO, encoding="utf-8")
            summary = ci_gates.summarize("tests", tmp, [("full", "failure"), ("addons", "success")])
        self.assertFalse(summary["ok"])
        self.assertIn("failing in full: `b`", ci_gates.markdown(summary))
        with tempfile.TemporaryDirectory() as tmp:
            self.assertTrue(ci_gates.summarize("checks", tmp, [("paths", "success")])["ok"])
        # Negative control: a skipped or cancelled gate is not a pass.
        for outcome in ("skipped", "cancelled", "", "failure"):
            with tempfile.TemporaryDirectory() as tmp:
                self.assertFalse(ci_gates.summarize("checks", tmp, [("paths", outcome)])["ok"])

    def test_summarize_command_writes_json_and_markdown(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "full.log").write_bytes(self.CARGO.encode("utf-16"))  # a Windows PowerShell 5 log
            out, md = Path(tmp, "s.json"), Path(tmp, "s.md")
            ci_gates.main(["summarize", "--job", "tests", "--logs", tmp, "--out", str(out),
                           "--markdown", str(md), "--result", "full=failure"])
            data = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(data["gates"][0]["skips"], 2)
            self.assertIn("| full | failure |", md.read_text(encoding="utf-8"))

    def test_status_text_lists_failures_and_skips(self):
        jobs = [{"name": "tests", "status": "completed", "conclusion": "failure",
                 "steps": [{"name": "full", "status": "completed", "conclusion": "success"},
                           {"name": "Set up job", "status": "completed", "conclusion": "success"}]}]
        summary = {"gates": [{"name": "full", "outcome": "failure", "passed": 5, "failed": 1, "ignored": 3,
                              "skips": 2, "failing": ["b"]}]}
        text = ci_gates.format_status(jobs, [summary])
        self.assertIn("tests: failure", text)
        self.assertIn("  full: failure; 5 passed, 1 failed, 3 ignored; 2 SKIP", text)
        self.assertIn("FAILING: b", text)
        # The raw step said success (continue-on-error); it must not be shown as the result.
        self.assertNotIn("full: success", text)
        self.assertNotIn("Set up job", text)
        # Without a summary the steps are shown, marked unverified.
        self.assertIn("tests/full: success (unverified)", ci_gates.format_status(jobs, []))


class WorkflowTests(unittest.TestCase):
    WORKFLOW = (Path(__file__).resolve().parents[1] / ".github/workflows/branch-gates.yml").read_text(encoding="utf-8")

    def test_single_line_run_values_hold_no_colon_space(self):
        # `run: throw 'x: y'` is a YAML mapping error: GitHub rejects the file with a 0s failed run.
        for line in self.WORKFLOW.splitlines():
            match = re.match(r"\s*(?:- )?(?:run|name): (?!['\"|>])(.+)$", line)
            if match:
                self.assertNotIn(": ", match.group(1), line)

    def test_logs_stay_outside_the_checkout(self):
        # Untracked files in the checkout fail the footprint gate (seen in the first CI trial).
        self.assertNotIn("-FilePath gate-logs", self.WORKFLOW)
        self.assertNotIn("--logs gate-logs", self.WORKFLOW)
        self.assertEqual(self.WORKFLOW.count("GATE_LOGS=$canonicalTemp"), 2)

    def test_workflow_cannot_publish_or_write(self):
        self.assertIn("permissions:\n  contents: read", self.WORKFLOW)
        self.assertNotIn("contents: write", self.WORKFLOW)
        for forbidden in ("secrets.", "gh release", "gh api", "git push", "git tag", "EQOA_UPDATE_CHANNEL",
                          "actions/upload-release", "dev_release.py publish", "GH_TOKEN"):
            self.assertNotIn(forbidden, self.WORKFLOW)

    def test_workflow_triggers_only_on_ci_branches_and_cancels_older_runs(self):
        self.assertIn("branches: ['ci/**']", self.WORKFLOW)
        triggers = self.WORKFLOW.split("\non:\n")[1].split("jobs:")[0]
        self.assertNotIn("develop", triggers)
        self.assertNotIn("enhanced", triggers)
        self.assertIn("cancel-in-progress: true", self.WORKFLOW)
        self.assertIn("workflow_dispatch:", self.WORKFLOW)

    def test_workflow_runs_every_named_gate_and_the_summary_names_match(self):
        for gate in ("fork_ext::addons::", "cargo +1.99.1 test --locked --workspace", "check_fork_footprint.py",
                     "check_no_local_paths.py", "test_player_release.py", "--target wasm32-unknown-unknown",
                     "cargo +1.99.1 check --locked -p eqoa 2>&1"):
            self.assertIn(gate, self.WORKFLOW)
        for name in ("addons", "full", "wasm", "program", "footprint", "paths", "player-release",
                     "unix-program", "unix-addons", "unix-extra"):
            self.assertIn(f"--result {name}=", self.WORKFLOW)
            self.assertIn(name, ci_gates.GATE_STEPS)

    def test_workflow_has_a_linux_and_a_macos_job_with_the_pinned_toolchain(self):
        self.assertIn("runner: ubuntu-22.04", self.WORKFLOW)
        self.assertIn("runner: macos-15", self.WORKFLOW)
        self.assertIn("libasound2-dev libudev-dev pkg-config", self.WORKFLOW)
        for job in ("linux", "macos"):
            self.assertIn(job, ci_gates.SUMMARY_JOBS)


if __name__ == "__main__":
    unittest.main()
