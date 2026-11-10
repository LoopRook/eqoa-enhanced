# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Source CI never publishes; only official-release.yml publishes official client bytes."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = "github.repository == 'DabDavis/eqoa-rust-client-vanilla'"


class PublicWorkflowTests(unittest.TestCase):
    def test_optimized_zero_addon_gate_is_mandatory_before_builds(self):
        gate = "play::fork_hooks::native_tests::performance_busy_world_zero_addons_frame_and_packet_allocation_bound"

        def check(workflow):
            section = workflow.split("- name: Require optimized zero-addon allocation gate\n", 1)[1]
            section, build = section.split("      - name: Build", 1)
            self.assertIn("cargo +1.99.1 test --release --locked -p eqoa --bin eqoa", section)
            self.assertIn(gate + " -- --exact --test-threads=1 --nocapture", section)
            self.assertIn("if ($LASTEXITCODE -ne 0) { throw", section)
            self.assertNotIn("continue-on-error", section)
            self.assertIn("cargo +1.99.1 build --release --locked -p eqoa", build)

        for name in ("public-source.yml", "enhanced-dev.yml"):
            with self.subTest(workflow=name):
                workflow = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
                check(workflow)
                # Negative controls: a debug run or swallowed failure cannot satisfy the gate.
                for broken in (workflow.replace("test --release --locked", "test --locked"),
                               workflow.replace("throw 'Optimized zero-addon", "Write-Host 'Optimized zero-addon")):
                    with self.assertRaises(AssertionError):
                        check(broken)

    def test_public_source_ci_has_no_publishing_credentials_or_artifacts(self):
        workflow = (ROOT / ".github/workflows/public-source.yml").read_text(encoding="utf-8")
        self.assertIn("contents: read", workflow)
        self.assertIn("tags: ['v*']", workflow)
        self.assertIn("pull_request:", workflow)
        self.assertIn("cargo +1.99.1 test --locked --workspace", workflow)
        self.assertIn("cargo +1.99.1 build --release --locked -p eqoa", workflow)
        for forbidden in ("contents: write", "secrets.", "gh release", "gh api",
                          "upload-artifact", "EQOA_UPDATE_CHANNEL", "git push"):
            self.assertNotIn(forbidden, workflow)

    def test_retired_publisher_is_inert(self):
        workflow = (ROOT / ".github/workflows/enhanced-release.yml").read_text(encoding="utf-8")
        self.assertIn("if: ${{ false }}", workflow)
        for forbidden in ("contents: write", "secrets.", "gh release", "upload-artifact", "git push"):
            self.assertNotIn(forbidden, workflow)

    def test_every_legacy_job_is_disabled_on_both_enhanced_repositories(self):
        for name in ("dev.yml", "notes.yml"):
            workflow = (ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            jobs = re.split(r"(?m)^  [a-z][a-z_-]*:\s*$", workflow.split("\njobs:\n", 1)[1])[1:]
            self.assertTrue(jobs, name)
            for job in jobs:
                guard = re.search(r"(?m)^    if: (.+)$", job)
                self.assertIsNotNone(guard, name)
                self.assertIn(UPSTREAM, guard.group(1), name)
                self.assertNotIn(" || ", guard.group(1), name)


if __name__ == "__main__":
    unittest.main()
