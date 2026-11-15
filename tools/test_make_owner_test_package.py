#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for make_owner_test_package.py."""
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile

import make_owner_test_package


class TestMakeOwnerTestPackage(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory(prefix="test-owner-pkg-")
        self.addCleanup(self.temp_dir.cleanup)
        self.base = Path(self.temp_dir.name)

        self.fake_root = self.base / "repo"
        self.fake_root.mkdir()

        self.fake_exe = self.base / "eqoa.exe"
        self.fake_exe.write_bytes(b"MZfake-exe")

        self.fake_config = self.base / "appdata" / "eqoa-rs"
        self.fake_config.mkdir(parents=True)
        (self.fake_config / "eqoa.ini").write_text("volume=100\n", encoding="utf-8")
        (self.fake_config / "binds.txt").write_text("bind 1 attack\n", encoding="utf-8")

        self.output_dir = self.base / "Downloads" / "EQOA-Enhanced-Round4-Test"

    def _fake_build_release(self, *, root, executable, output, channel="enhanced-dev", **kwargs):
        # Create a fake zip in output and return it
        stage = Path(output)
        stage.mkdir(parents=True, exist_ok=True)
        zip_path = stage / "release.zip"
        exe_title = "EQOA Enhanced - Nightly.exe" if channel == "enhanced-dev" else "EQOA Enhanced.exe"
        with zipfile.ZipFile(zip_path, "w") as z:
            z.writestr(exe_title, b"MZfake-channel-exe")
            z.writestr("map-entities.json", b"{}")
            z.writestr("mods/README.txt", b"mods")
        return zip_path

    def test_folder_layout_and_both_bats_exact_text(self):
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            out = make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                channel="enhanced-dev",
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
            )

        self.assertEqual(out, self.output_dir)
        self.assertTrue((self.output_dir / "config").is_dir())
        self.assertTrue((self.output_dir / "play").is_dir())
        self.assertTrue((self.output_dir / "play" / "EQOA Enhanced - Nightly.exe").is_file())

        normal_bat = self.output_dir / "Play EQOA Round4 Test.bat"
        fps_bat = self.output_dir / "Play EQOA Round4 Test (fps log).bat"

        self.assertTrue(normal_bat.is_file())
        self.assertTrue(fps_bat.is_file())

        expected_normal = (
            "@echo off\r\n"
            "rem Round 4 test build: uses the settings copy in .\\config, never updates itself.\r\n"
            "set EQOA_CONFIG=%~dp0config\r\n"
            "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
            'start "" /D "%~dp0play" "%~dp0play\\EQOA Enhanced - Nightly.exe"\r\n'
        )

        expected_fps = (
            "@echo off\r\n"
            "rem Round 4 test build: uses the settings copy in .\\config, never updates itself.\r\n"
            "set EQOA_CONFIG=%~dp0config\r\n"
            "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
            "set EQOA_PERF=1\r\n"
            'start "" /D "%~dp0play" "%~dp0play\\EQOA Enhanced - Nightly.exe"\r\n'
        )

        self.assertEqual(normal_bat.read_bytes().decode("ascii"), expected_normal)
        self.assertEqual(fps_bat.read_bytes().decode("ascii"), expected_fps)

    def test_config_copied(self):
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                channel="enhanced-dev",
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
            )

        self.assertTrue((self.output_dir / "config" / "eqoa.ini").is_file())
        self.assertEqual(
            (self.output_dir / "config" / "eqoa.ini").read_text(encoding="utf-8"),
            "volume=100\n",
        )
        self.assertEqual(
            (self.output_dir / "config" / "binds.txt").read_text(encoding="utf-8"),
            "bind 1 attack\n",
        )

    def test_refusal_without_force(self):
        self.output_dir.mkdir(parents=True)
        (self.output_dir / "dummy.txt").write_text("existing", encoding="utf-8")

        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            with self.assertRaises(ValueError):
                make_owner_test_package.make_package(
                    name="Round4",
                    exe=self.fake_exe,
                    output_dir=self.output_dir,
                    root=self.fake_root,
                    force=False,
                    config_source=self.fake_config,
                )

    def test_force_keeps_config_unless_fresh(self):
        # First creation
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
            )

        # Owner customizes a file in config
        custom_file = self.output_dir / "config" / "custom_owner_settings.json"
        custom_file.write_text('{"custom": true}', encoding="utf-8")

        # Second creation with --force (keeps config)
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.output_dir,
                root=self.fake_root,
                force=True,
                fresh_config=False,
                config_source=self.fake_config,
            )

        self.assertTrue(custom_file.is_file(), "--force should preserve existing config")

        # Third creation with --force and --fresh-config (resets config)
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.output_dir,
                root=self.fake_root,
                force=True,
                fresh_config=True,
                config_source=self.fake_config,
            )

        self.assertFalse(custom_file.is_file(), "--fresh-config should overwrite existing config")
        self.assertTrue((self.output_dir / "config" / "eqoa.ini").is_file())


if __name__ == "__main__":
    unittest.main()
