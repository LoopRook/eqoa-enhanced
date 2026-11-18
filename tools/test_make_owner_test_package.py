#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for make_owner_test_package.py."""
import io
import os
from pathlib import Path
import subprocess
import sys
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

        self.fake_appdata = self.base / "fake_appdata"
        self.fake_config = self.fake_appdata / "eqoa-rs"
        self.fake_config.mkdir(parents=True)
        (self.fake_config / "eqoa.ini").write_text("volume=100\n", encoding="utf-8")
        (self.fake_config / "binds.txt").write_text("bind 1 attack\n", encoding="utf-8")

        self.env_patch = mock.patch.dict(os.environ, {"APPDATA": str(self.fake_appdata)})
        self.env_patch.start()
        self.addCleanup(self.env_patch.stop)

        self.output_dir = self.base / "Downloads" / "EQOA-Enhanced-Round4-Test"

    def _fake_build_release(self, *, root, executable, output, channel="enhanced-dev", **kwargs):
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
            'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0archive_previous_run.ps1"\r\n'
            'start "" /D "%~dp0play" "%~dp0play\\EQOA Enhanced - Nightly.exe"\r\n'
        )

        expected_fps = (
            "@echo off\r\n"
            "rem Round 4 test build: uses the settings copy in .\\config, never updates itself.\r\n"
            "set EQOA_CONFIG=%~dp0config\r\n"
            "set EQOA_UPDATE_URL=http://127.0.0.1:9/\r\n"
            "set EQOA_PERF=1\r\n"
            'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0archive_previous_run.ps1"\r\n'
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

    def test_refusal_when_output_overlaps_live_config(self):
        # 1. output_dir equals live config
        with self.assertRaises(make_owner_test_package.LiveConfigOverlapError):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.fake_config,
                root=self.fake_root,
            )

        # 2. output_dir inside live config
        nested_in_live = self.fake_config / "subfolder" / "pkg"
        with self.assertRaises(make_owner_test_package.LiveConfigOverlapError):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=nested_in_live,
                root=self.fake_root,
            )

        # 3. output_dir contains live config (ancestor)
        with self.assertRaises(make_owner_test_package.LiveConfigOverlapError):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.fake_appdata,
                root=self.fake_root,
            )

    def test_refusal_when_config_is_symlink(self):
        self.output_dir.mkdir(parents=True)
        config_dir = self.output_dir / "config"
        config_dir.mkdir(parents=True)

        with mock.patch("make_owner_test_package.is_symlink_or_junction", return_value=True):
            with self.assertRaises(make_owner_test_package.LiveConfigOverlapError):
                make_owner_test_package.make_package(
                    name="Round4",
                    exe=self.fake_exe,
                    output_dir=self.output_dir,
                    root=self.fake_root,
                    force=True,
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

    def test_force_staging_failure_leaves_existing_play_untouched(self):
        # Create an existing working package
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            make_owner_test_package.make_package(
                name="Round4",
                exe=self.fake_exe,
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
            )

        sentinel = self.output_dir / "play" / "previous_working_build.txt"
        sentinel.write_text("owner working build state", encoding="utf-8")

        # Now simulate a broken build with --force
        def _failing_build(**kwargs):
            raise RuntimeError("Build compilation error or missing asset")

        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=_failing_build):
            with self.assertRaises(RuntimeError):
                make_owner_test_package.make_package(
                    name="Round4",
                    exe=self.fake_exe,
                    output_dir=self.output_dir,
                    root=self.fake_root,
                    force=True,
                    config_source=self.fake_config,
                )

        # The previous play/ directory and sentinel MUST remain untouched!
        self.assertTrue(sentinel.is_file())
        self.assertEqual(sentinel.read_text("utf-8"), "owner working build state")

    def test_build_env_command_lock_and_summary(self):
        fake_home = self.base / "user_home"
        expected_target = fake_home / "Paseo" / "targets" / "shared"
        built_exe = expected_target / "release" / "eqoa.exe"
        built_exe.parent.mkdir(parents=True, exist_ok=True)
        built_exe.write_bytes(b"MZbuilt-exe")

        fake_commit = "a" * 40
        recorded_calls = []
        lock_events = []

        class FakeLock:
            def __enter__(self):
                lock_events.append("acquire")
                return self

            def __exit__(self, exc_type, exc_val, exc_tb):
                lock_events.append("release")
                return False

        def fake_run(cmd, **kwargs):
            recorded_calls.append((cmd, kwargs))
            if cmd[0] == "git" and "status" in cmd:
                return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
            if cmd[0] == "git" and "rev-parse" in cmd:
                return subprocess.CompletedProcess(cmd, 0, stdout=f"{fake_commit}\n", stderr="")
            if cmd[0] == "cargo":
                self.assertEqual(lock_events, ["acquire"])
                return subprocess.CompletedProcess(
                    cmd,
                    0,
                    stdout="",
                    stderr=(
                        "   Compiling eqoa v0.1.0 (C:/repo/crates/eqoa)\n"
                        "    Finished `release` profile [optimized] target(s) in 12.34s\n"
                    ),
                )
            raise AssertionError(f"Unexpected command: {cmd}")

        clean_env = {k: v for k, v in os.environ.items() if k not in ("CARGO_TARGET_DIR", "RUSTFLAGS", "EQOA_BUILD_COMMIT", "EQOA_UPDATE_CHANNEL")}
        clean_env["USERPROFILE"] = str(fake_home)
        clean_env["APPDATA"] = str(self.fake_appdata)

        stdout_buf = io.StringIO()
        with (
            mock.patch.dict(os.environ, clean_env, clear=True),
            mock.patch("pathlib.Path.home", return_value=fake_home),
            mock.patch("make_owner_test_package.run_gates.CargoLock", return_value=FakeLock()),
            mock.patch("make_owner_test_package.subprocess.run", side_effect=fake_run),
            mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release) as mock_build_release,
            mock.patch("sys.stdout", stdout_buf),
        ):
            out = make_owner_test_package.make_package(
                name="Round6",
                build=True,
                channel="enhanced-dev",
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
            )

        self.assertEqual(out, self.output_dir)
        self.assertEqual(lock_events, ["acquire", "release"])
        cargo_calls = [(cmd, kw) for cmd, kw in recorded_calls if cmd[0] == "cargo"]
        self.assertEqual(len(cargo_calls), 1)
        cargo_cmd, cargo_kw = cargo_calls[0]
        self.assertEqual(cargo_cmd, ["cargo", "build", "--locked", "--release", "-p", "eqoa"])
        self.assertEqual(cargo_kw["cwd"], self.fake_root.resolve())
        cargo_env = cargo_kw["env"]
        self.assertEqual(cargo_env["CARGO_TARGET_DIR"], str(expected_target))
        self.assertEqual(cargo_env["EQOA_BUILD_COMMIT"], fake_commit)
        self.assertEqual(cargo_env["EQOA_UPDATE_CHANNEL"], "enhanced-dev")
        self.assertEqual(cargo_env["RUSTFLAGS"], "-C target-feature=+crt-static")
        self.assertEqual(mock_build_release.call_args.kwargs["executable"], built_exe.resolve())

        printed = stdout_buf.getvalue()
        self.assertIn("Finished `release` profile", printed)
        self.assertNotIn("Compiling eqoa", printed)

    def test_build_releases_lock_on_cargo_failure(self):
        lock_events = []

        class FakeLock:
            def __enter__(self):
                lock_events.append("acquire")
                return self

            def __exit__(self, exc_type, exc_val, exc_tb):
                lock_events.append("release")
                return False

        def fake_run(cmd, **kwargs):
            if cmd[0] == "git" and "status" in cmd:
                return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")
            if cmd[0] == "git" and "rev-parse" in cmd:
                return subprocess.CompletedProcess(cmd, 0, stdout="b" * 40 + "\n", stderr="")
            if cmd[0] == "cargo":
                return subprocess.CompletedProcess(cmd, 101, stdout="", stderr="error: could not compile `eqoa`\n")
            raise AssertionError(f"Unexpected command: {cmd}")

        with (
            mock.patch("make_owner_test_package.run_gates.CargoLock", return_value=FakeLock()),
            mock.patch("make_owner_test_package.subprocess.run", side_effect=fake_run),
            mock.patch("sys.stdout", io.StringIO()),
        ):
            with self.assertRaises(RuntimeError):
                make_owner_test_package.make_package(
                    name="Round6",
                    build=True,
                    output_dir=self.output_dir,
                    root=self.fake_root,
                    config_source=self.fake_config,
                )

        self.assertEqual(lock_events, ["acquire", "release"])

    def test_build_refuses_dirty_tracked_tree(self):
        def fake_run(cmd, **kwargs):
            if cmd[0] == "git" and "status" in cmd:
                return subprocess.CompletedProcess(cmd, 0, stdout=" M crates/eqoa/src/main.rs\n", stderr="")
            raise AssertionError(f"Should not run further commands on dirty tree: {cmd}")

        with (
            mock.patch("make_owner_test_package.run_gates.CargoLock") as mock_lock,
            mock.patch("make_owner_test_package.subprocess.run", side_effect=fake_run),
        ):
            with self.assertRaisesRegex(ValueError, "uncommitted changes"):
                make_owner_test_package.make_package(
                    name="Round6",
                    build=True,
                    output_dir=self.output_dir,
                    root=self.fake_root,
                    config_source=self.fake_config,
                )

        mock_lock.assert_not_called()

    def test_exe_and_build_mutually_exclusive(self):
        with self.assertRaisesRegex(ValueError, "Cannot specify both"):
            make_owner_test_package.make_package(
                name="Round6",
                exe=self.fake_exe,
                build=True,
                output_dir=self.output_dir,
                root=self.fake_root,
            )

        with mock.patch.object(sys, "argv", ["make_owner_test_package.py", "--exe", str(self.fake_exe), "--build"]):
            with mock.patch("sys.stderr", io.StringIO()):
                with self.assertRaises(SystemExit) as cm:
                    make_owner_test_package.main()
                self.assertEqual(cm.exception.code, 2)

        with mock.patch.object(sys, "argv", ["make_owner_test_package.py"]):
            with mock.patch("sys.stderr", io.StringIO()):
                with self.assertRaises(SystemExit) as cm:
                    make_owner_test_package.main()
                self.assertEqual(cm.exception.code, 2)


    def _make(self, **kwargs):
        with mock.patch("make_owner_test_package.build_player_release.build_release", side_effect=self._fake_build_release):
            return make_owner_test_package.make_package(
                name="Round7",
                exe=self.fake_exe,
                output_dir=self.output_dir,
                root=self.fake_root,
                config_source=self.fake_config,
                **kwargs,
            )

    def _run_archive(self):
        subprocess.run(
            ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
             str(self.output_dir / "archive_previous_run.ps1")],
            check=True, capture_output=True,
        )

    @unittest.skipUnless(sys.platform == "win32", "the launcher helper is PowerShell on Windows")
    def test_launcher_sets_each_run_aside_and_keeps_account_files(self):
        self._make()
        config = self.output_dir / "config"
        (config / "account1.opt").write_text("secret-login", encoding="utf-8")
        (config / "acct_main.opt").write_text("secret-two", encoding="utf-8")
        (config / "eqoa.log").write_text("run one\n", encoding="utf-8")
        (config / "state.txt").write_text("state one\n", encoding="utf-8")
        (config / "addons").mkdir()
        (config / "addons" / "addons_error.log").write_text("mod broke\n", encoding="utf-8")

        self._run_archive()
        (config / "eqoa.log").write_text("run two\n", encoding="utf-8")
        self._run_archive()

        self.assertFalse((config / "eqoa.log").exists())
        self.assertFalse((config / "state.txt").exists())
        runs = sorted((self.output_dir / "results").iterdir())
        self.assertEqual(len(runs), 2, "a second launch must not erase the first run")
        texts = sorted(
            (r / "config" / "eqoa.log").read_text(encoding="utf-8")
            for r in runs if (r / "config" / "eqoa.log").exists()
        )
        self.assertEqual(texts, ["run one\n", "run two\n"])
        self.assertTrue(any((r / "config" / "addons" / "addons_error.log").is_file() for r in runs))
        self.assertTrue(any((r / "config" / "state.txt").is_file() for r in runs))
        # credentials stay where they were and are never copied
        self.assertEqual((config / "account1.opt").read_text(encoding="utf-8"), "secret-login")
        self.assertTrue((config / "acct_main.opt").is_file())
        moved = [p.name for p in (self.output_dir / "results").rglob("*") if p.is_file()]
        self.assertFalse([n for n in moved if n.endswith(".opt")], moved)

    @unittest.skipUnless(sys.platform == "win32", "the launcher helper is PowerShell on Windows")
    def test_launcher_with_nothing_to_set_aside_makes_no_results_folder(self):
        self._make()
        (self.output_dir / "config" / "eqoa.ini").write_text("x", encoding="utf-8")
        self._run_archive()
        self.assertFalse((self.output_dir / "results").exists())

    def test_checklist_copied_as_checklist_txt(self):
        checklist = self.base / "round7-list.txt"
        checklist.write_text("1. Log in. You see the world.\n", encoding="utf-8")
        self._make(checklist=checklist)
        self.assertEqual(
            (self.output_dir / "CHECKLIST.txt").read_text(encoding="utf-8"),
            "1. Log in. You see the world.\n",
        )

    def test_package_without_checklist_has_none(self):
        self._make()
        self.assertFalse((self.output_dir / "CHECKLIST.txt").exists())

    def test_missing_checklist_refused_before_anything_is_written(self):
        with self.assertRaises(FileNotFoundError):
            self._make(checklist=self.base / "nope.txt")
        self.assertFalse(self.output_dir.exists())


if __name__ == "__main__":
    unittest.main()
