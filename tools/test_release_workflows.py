"""The public fork validates source but never publishes official client bytes."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
UPSTREAM = "github.repository == 'DabDavis/eqoa-rust-client-vanilla'"


class PublicWorkflowTests(unittest.TestCase):
    def test_public_source_ci_has_no_publishing_credentials_or_artifacts(self):
        workflow = (ROOT / ".github/workflows/enhanced-release.yml").read_text(encoding="utf-8")
        self.assertIn("contents: read", workflow)
        self.assertIn("tags: ['v*']", workflow)
        self.assertIn("pull_request:", workflow)
        self.assertIn("cargo +1.99.1 test --locked --workspace", workflow)
        self.assertIn("cargo +1.99.1 build --release --locked -p eqoa", workflow)
        for forbidden in ("contents: write", "secrets.", "gh release", "gh api",
                          "upload-artifact", "EQOA_UPDATE_CHANNEL", "git push"):
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
