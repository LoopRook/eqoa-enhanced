"""Remote draft allowlist and monotonic semantic release contracts."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check_release_assets.py")
EXPECTED = ["EQOA-Enhanced-0.1.0-Windows-x64.zip", "eqoa-enhanced-windows-x86_64.exe", "update.txt", "update-notes.md", "SHA256SUMS"]


class ReleaseAssetTests(unittest.TestCase):
    def run_guard(self, mode, payload, *flags, version="0.1.0"):
        self.assertTrue(SCRIPT.is_file(), "remote release asset guard has not been implemented")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "remote.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            return subprocess.run([sys.executable, str(SCRIPT), mode, str(path), "--version", version, *flags], text=True, capture_output=True)

    def draft(self, names, is_draft=True):
        return {"isDraft": is_draft, "assets": [{"name": name} for name in names]}

    def test_exact_remote_draft_is_accepted(self):
        result = self.run_guard("draft", self.draft(EXPECTED))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_assets_allowed_only_before_upload(self):
        for names in ([], EXPECTED[:2]):
            with self.subTest(names=names):
                self.assertEqual(self.run_guard("draft", self.draft(names), "--allow-missing").returncode, 0)
                self.assertNotEqual(self.run_guard("draft", self.draft(names)).returncode, 0)

    def test_extra_private_assets_always_rejected(self):
        for extra in ("profile/accounts.txt", "owned.iso", "old-preview.zip", "source.tar.gz", "../secret", "player.cmd"):
            for flags in ((), ("--allow-missing",)):
                with self.subTest(extra=extra, flags=flags):
                    result = self.run_guard("draft", self.draft(EXPECTED + [extra]), *flags)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("Unexpected", result.stderr)

    def test_published_release_and_duplicate_assets_are_rejected(self):
        self.assertNotEqual(self.run_guard("draft", self.draft(EXPECTED, is_draft=False), "--allow-missing").returncode, 0)
        self.assertNotEqual(self.run_guard("draft", self.draft(EXPECTED + EXPECTED[:1])).returncode, 0)

    def test_new_version_must_exceed_all_published_semantic_versions(self):
        older = [{"tagName": "v0.9.9", "isDraft": False}, {"tagName": "v0.20.0", "isDraft": True}, {"tagName": "dev", "isDraft": False}]
        result = self.run_guard("history", older, version="0.10.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        for tag in ("v0.10.0", "v0.10.1", "v1.0.0"):
            with self.subTest(tag=tag):
                self.assertNotEqual(self.run_guard("history", older + [{"tagName": tag, "isDraft": False}], version="0.10.0").returncode, 0)

    def test_malformed_versions_and_remote_payloads_fail_closed(self):
        for version in ("01.1.0", "v0.1.0", "0.1.0-rc1", "0.1"):
            with self.subTest(version=version):
                self.assertNotEqual(self.run_guard("draft", self.draft(EXPECTED), version=version).returncode, 0)
        for payload in ({}, [], {"isDraft": True, "assets": ["accounts.txt"]}):
            with self.subTest(payload=payload):
                self.assertNotEqual(self.run_guard("draft", payload).returncode, 0)


if __name__ == "__main__":
    unittest.main()
