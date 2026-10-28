"""Exercise the footprint CLI against disposable, real Git repositories."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

CHECKER = Path(__file__).with_name("check_fork_footprint.py")


class FootprintTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "footprint@example.invalid")
        self.git("config", "user.name", "Footprint test")
        self.git("config", "core.autocrlf", "false")
        self.write("shared.rs", "upstream();\n")
        self.write("other.rs", "untouched();\n")
        self.write("LICENSE", "upstream license\n")
        self.write("UPDATE-NOTES.md", "upstream notes\n")
        self.write("CLAUDE.md", "Vanilla rules\n")
        self.write("Cargo.toml", '[package]\nname = "fixture"\nlicense = "MIT"\n')
        self.git("add", ".")
        self.git("commit", "-qm", "upstream")
        self.git("update-ref", "refs/remotes/origin/master", "HEAD")
        self.manifest = {
            "schema_version": 1,
            "fork_owned": ["docs/**", "README.md", "tools/owned.py"],
            "shared": {"shared.rs": {"added": 1, "deleted": 0, "reason": "one hook"},
                       "Cargo.toml": {"added": 1, "deleted": 0, "reason": "dependency"}},
            "immutable": ["LICENSE", "UPDATE-NOTES.md"],
            "claude_override": "On this fork, FORK.md overrides Vanilla rules.",
            "license_manifests": ["Cargo.toml"],
            "hooks": [{"path": "shared.rs", "pattern": "fork_hook\\(", "count": 1}],
        }
        self.write("CLAUDE.md", "Vanilla rules\n\n" + self.manifest["claude_override"] + "\n")
        self.write("shared.rs", "upstream();\nfork_hook();\n")

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), *args], check=True,
                              capture_output=True, text=True).stdout

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def check(self, *args):
        self.write("docs/policy.json", json.dumps(self.manifest))
        return subprocess.run([sys.executable, "-Xutf8", str(CHECKER), "--root", str(self.root),
                               "--manifest", "docs/policy.json", *args],
                              capture_output=True, text=True, encoding="utf-8")

    def test_allowed_working_tree_staged_and_untracked_changes_pass(self):
        self.git("add", "shared.rs")
        self.write("README.md", "Enhanced landing page\n")
        self.write("docs/new.md", "Fork note\n")
        result = self.check("--show-diff")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("+1 -0", result.stdout)
        self.assertIn("+fork_hook();", result.stdout)

    def test_over_budget_shared_change_fails(self):
        self.write("shared.rs", "upstream();\nfork_hook();\nextra();\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exceeds budget", result.stdout)

    def test_deletion_budget_is_enforced_independently(self):
        self.write("shared.rs", "fork_hook();\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exceeds budget", result.stdout)

    def test_allowlisted_untracked_shared_file_prints_its_added_lines(self):
        self.manifest["shared"]["new.rs"] = {"added": 1, "deleted": 0, "reason": "reviewed adapter"}
        self.write("new.rs", "new_adapter();\n")
        result = self.check("--show-diff")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("+new_adapter();", result.stdout)

    def test_unexpected_tracked_shared_path_fails(self):
        self.write("other.rs", "new_behavior();\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unreviewed shared path: other.rs", result.stdout)

    def test_unexpected_untracked_path_fails(self):
        self.write("surprise.rs", "new_behavior();\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unreviewed shared path: surprise.rs", result.stdout)

    def test_duplicate_hook_fails_even_with_room_in_budget(self):
        self.manifest["shared"]["shared.rs"]["added"] = 3
        self.write("shared.rs", "upstream();\nfork_hook();\nfork_hook();\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("hook count", result.stdout)

    def test_license_and_upstream_notes_are_immutable(self):
        for path in ("LICENSE", "UPDATE-NOTES.md"):
            with self.subTest(path=path):
                original = (self.root / path).read_text(encoding="utf-8")
                self.write(path, "changed\n")
                result = self.check()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("immutable upstream file changed: " + path, result.stdout)
                self.write(path, original)

    def test_claude_only_allows_exact_scope_override(self):
        self.write("CLAUDE.md", "Different rules\n\n" + self.manifest["claude_override"] + "\n")
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CLAUDE.md must equal upstream", result.stdout)

    def test_cargo_license_cannot_change_within_shared_budget(self):
        self.manifest["shared"]["Cargo.toml"].update(added=5, deleted=5)
        self.write("Cargo.toml", '[package]\nname = "fixture"\nlicense = "Apache-2.0"\n')
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Cargo license changed", result.stdout)

    def test_negative_budget_and_broad_shared_allowlist_are_rejected(self):
        self.manifest["shared"]["*.rs"] = {"added": -1, "deleted": 0, "reason": "invalid"}
        result = self.check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid shared budget", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
