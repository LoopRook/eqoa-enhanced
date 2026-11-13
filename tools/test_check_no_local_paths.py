# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""The local-path guard reports a profile path and accepts the placeholders."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "tools" / "check_no_local_paths.py"
# Built by concatenation so this fixture is not itself a local path in the repository.
PROFILE_PATH = "C:" + "\\" + "Users" + "\\" + "sample" + "\\eqoa.iso"
PLACEHOLDERS = """builds use %USERPROFILE%/Paseo/targets/shared under the lock
`<worktree:forge-compatibility>` and `<repo>` are checkouts
the disc at $EQOA_ISO; logs in %LOCALAPPDATA%/Temp/opencode
"""


class LocalPathGuardTests(unittest.TestCase):
    def check(self, files, untracked=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, data in {**files, **(untracked or {})}.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data if isinstance(data, bytes) else data.encode("utf-8"))
            subprocess.run(["git", "init", "-q", str(root)], check=True, capture_output=True)
            subprocess.run(["git", "-C", str(root), "add", "--", *files], check=True, capture_output=True)
            return subprocess.run([sys.executable, str(SCRIPT), str(root)], text=True, capture_output=True)

    def test_local_profile_path_is_reported(self):
        result = self.check({"docs/note.md": f"the owner's disc: {PROFILE_PATH}\n"})
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("docs/note.md:1:", result.stdout)
        self.assertIn("user-profile path", result.stdout)

    def test_placeholders_pass(self):
        result = self.check({"docs/note.md": PLACEHOLDERS})
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("No local user-profile paths", result.stdout)

    def test_only_tracked_text_files_are_scanned(self):
        result = self.check(
            {"docs/bin.dat": b"\0\0" + PROFILE_PATH.encode("utf-8")},
            untracked={"docs/untracked.md": PROFILE_PATH + "\n"},
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
