#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for update_bundled_manifest.py."""
import hashlib
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import update_bundled_manifest


class TestUpdateBundledManifest(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory(prefix="test-update-manifest-")
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name)

        repo_root = Path(__file__).resolve().parent.parent

        shutil.copytree(repo_root / "bundled-mods", self.root / "bundled-mods")
        (self.root / "tools").mkdir()
        shutil.copy2(repo_root / "tools" / "build_player_release.py", self.root / "tools" / "build_player_release.py")
        shutil.copy2(repo_root / "tools" / "test_player_release.py", self.root / "tools" / "test_player_release.py")

        # Initialize a git repo in the fixture
        subprocess.run(["git", "init", "-q"], cwd=str(self.root), check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@test.local", "add", "."], cwd=str(self.root), check=True)
        subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@test.local", "commit", "-qm", "initial"], cwd=str(self.root), check=True)

    def test_clean_branch_check_passes_and_byte_identical(self):
        source_before = (self.root / "bundled-mods" / "SOURCE.txt").read_bytes()
        build_before = (self.root / "tools" / "build_player_release.py").read_bytes()
        test_before = (self.root / "tools" / "test_player_release.py").read_bytes()

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=True)
        self.assertEqual(exit_code, 0)

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(exit_code, 0)

        self.assertEqual((self.root / "bundled-mods" / "SOURCE.txt").read_bytes(), source_before)
        self.assertEqual((self.root / "tools" / "build_player_release.py").read_bytes(), build_before)
        self.assertEqual((self.root / "tools" / "test_player_release.py").read_bytes(), test_before)

    def test_new_file_added_to_all_three_and_unrelated_text_preserved(self):
        new_file = self.root / "bundled-mods" / "arpg_ui" / "new_asset.png"
        new_content = b"fake-png-content-12345"
        new_file.write_bytes(new_content)
        new_hash = hashlib.sha256(new_content).hexdigest()

        # Check should report differences
        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=True)
        self.assertEqual(exit_code, 1)

        orig_source_header = (self.root / "bundled-mods" / "SOURCE.txt").read_text(encoding="utf-8").split("SHA-256 checksums:\n")[0]
        orig_build_text = (self.root / "tools" / "build_player_release.py").read_text(encoding="utf-8")
        orig_build_prefix = orig_build_text.split("BUNDLED_MOD_FILES = (")[0]
        orig_build_suffix = orig_build_text.split("BUNDLED_MOD_FILES = (")[1].split("\n)\n")[1]

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(exit_code, 0)

        source_text = (self.root / "bundled-mods" / "SOURCE.txt").read_text(encoding="utf-8")
        self.assertIn(f"{new_hash}  arpg_ui/new_asset.png", source_text)
        new_source_header = source_text.split("SHA-256 checksums:\n")[0]
        self.assertEqual(orig_source_header, new_source_header)

        build_text = (self.root / "tools" / "build_player_release.py").read_text(encoding="utf-8")
        self.assertIn('"arpg_ui/new_asset.png",', build_text)
        new_build_prefix = build_text.split("BUNDLED_MOD_FILES = (")[0]
        new_build_suffix = build_text.split("BUNDLED_MOD_FILES = (")[1].split("\n)\n")[1]
        self.assertEqual(orig_build_prefix, new_build_prefix)
        self.assertEqual(orig_build_suffix, new_build_suffix)

        test_text = (self.root / "tools" / "test_player_release.py").read_text(encoding="utf-8")
        self.assertIn('"arpg_ui/new_asset.png",', test_text)

    def test_changed_file_gets_new_hash(self):
        target_file = self.root / "bundled-mods" / "arpg_ui" / "README.md"
        new_content = b"Brand new README content for arpg_ui!"
        target_file.write_bytes(new_content)
        new_hash = hashlib.sha256(new_content).hexdigest()

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(exit_code, 0)

        source_text = (self.root / "bundled-mods" / "SOURCE.txt").read_text(encoding="utf-8")
        self.assertIn(f"{new_hash}  arpg_ui/README.md", source_text)

    def test_removed_file_disappears_from_all_three(self):
        target_file = self.root / "bundled-mods" / "arpg_ui" / "resources" / "BlankSlot.png"
        self.assertTrue(target_file.is_file())
        target_file.unlink()

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(exit_code, 0)

        source_text = (self.root / "bundled-mods" / "SOURCE.txt").read_text(encoding="utf-8")
        build_text = (self.root / "tools" / "build_player_release.py").read_text(encoding="utf-8")
        test_text = (self.root / "tools" / "test_player_release.py").read_text(encoding="utf-8")

        self.assertNotIn("arpg_ui/resources/BlankSlot.png", source_text)
        self.assertNotIn("arpg_ui/resources/BlankSlot.png", build_text)
        self.assertNotIn("arpg_ui/resources/BlankSlot.png", test_text)

    def test_stray_tmp_and_backup_files_are_ignored(self):
        # Create stray files that should NOT be added
        (self.root / "bundled-mods" / "arpg_ui" / "notes.tmp").write_text("temporary note\n")
        (self.root / "bundled-mods" / "arpg_ui" / "editor.lua~").write_text("backup\n")
        (self.root / "bundled-mods" / "root_stray.txt").write_text("stray in root\n")

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=True)
        # Should be 0 because stray files are ignored and no valid mod files changed
        self.assertEqual(exit_code, 0)

        source_text = (self.root / "bundled-mods" / "SOURCE.txt").read_text(encoding="utf-8")
        build_text = (self.root / "tools" / "build_player_release.py").read_text(encoding="utf-8")
        self.assertNotIn("notes.tmp", source_text)
        self.assertNotIn("editor.lua~", source_text)
        self.assertNotIn("root_stray.txt", source_text)
        self.assertNotIn("notes.tmp", build_text)

    def test_text_after_checksum_block_preserved_in_source_txt(self):
        source_path = self.root / "bundled-mods" / "SOURCE.txt"
        orig_text = source_path.read_text(encoding="utf-8")
        trailer = "\n# Trailing footer notes\nKeep this footer exactly.\n"
        source_path.write_text(orig_text + trailer, encoding="utf-8")

        # Add a new file to trigger rewrite
        (self.root / "bundled-mods" / "arpg_ui" / "another_mod.lua").write_text("print('test')\n")

        exit_code = update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(exit_code, 0)

        updated_text = source_path.read_text(encoding="utf-8")
        self.assertIn("arpg_ui/another_mod.lua", updated_text)
        self.assertTrue(updated_text.endswith(trailer), "Trailer after checksum block must be preserved")

    def test_byte_preservation_for_crlf_and_lf(self):
        source_path = self.root / "bundled-mods" / "SOURCE.txt"
        
        # Test with LF
        lf_bytes = source_path.read_bytes().replace(b"\r\n", b"\n")
        source_path.write_bytes(lf_bytes)
        update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(source_path.read_bytes(), lf_bytes)

        # Test with CRLF
        crlf_bytes = lf_bytes.replace(b"\n", b"\r\n")
        source_path.write_bytes(crlf_bytes)
        update_bundled_manifest.update_bundled_manifests(root=self.root, check=False)
        self.assertEqual(source_path.read_bytes(), crlf_bytes)


if __name__ == "__main__":
    unittest.main()
