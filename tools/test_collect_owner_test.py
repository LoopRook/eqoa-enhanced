#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Unit tests for collect_owner_test.py."""
import io
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest import mock

import collect_owner_test

PERF = "perf: {fps} fps, cpu 3.10 ms/frame (worst {worst}, 0 of 50 ms or more, 0 of 100), 120 models in 40 draws: x"
OPCODE = "play: {op} ({n} bytes) in mode 2"


class TestCollector(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="test-collect-")
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name)
        self.pkg = self.base / "pkg"
        (self.pkg / "config").mkdir(parents=True)
        (self.pkg / "play").mkdir()
        (self.pkg / "play" / "release-manifest.json").write_text(
            json.dumps({"version": "0.9.1", "sourceCommit": "abcdef0123456789" + "0" * 24, "updaterChannel": "enhanced-dev"}),
            encoding="utf-8",
        )
        (self.pkg / "config" / "server.opt").write_text("eqoaclassic.com\n", encoding="utf-8")
        (self.pkg / "Play X Test.bat").write_text("rem", encoding="utf-8")
        self.desktop = self.base / "Desktop"
        self.desktop.mkdir()

    def log(self, lines, where=None):
        folder = where or self.pkg / "config"
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "eqoa.log").write_text("\n".join(lines) + "\n", encoding="utf-8")

    def test_page_has_build_server_fps_opcodes(self):
        self.log([
            PERF.format(fps="60.0", worst="18.2"),
            PERF.format(fps="12.5", worst="95.0"),
            PERF.format(fps="58.0", worst="20.0"),
            OPCODE.format(op="0x0016", n=12),
            OPCODE.format(op="0x0018", n=40),
            OPCODE.format(op="0x0016", n=12),
        ])
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertIn("build: 0.9.1 commit abcdef012345 channel enhanced-dev", page)
        self.assertIn("server: eqoaclassic.com", page)
        self.assertIn("lowest 12.5, typical 58.0, worst frame 95.0 ms", page)
        self.assertIn("0x0016 x2, 0x0018 x1", page)
        self.assertIn("order: 0x0016(12b) 0x0018(40b) 0x0016(12b)", page)

    def test_no_perf_lines_says_so(self):
        self.log(["[eqoa] started"])
        self.assertIn("no perf lines", collect_owner_test.collect(self.pkg, self.desktop))

    def test_earlier_runs_listed_newest_first(self):
        self.log([PERF.format(fps="30.0", worst="40.0")])
        self.log([PERF.format(fps="11.0", worst="90.0")], self.pkg / "results" / "20261101-100000" / "config")
        self.log([PERF.format(fps="22.0", worst="50.0")], self.pkg / "results" / "20261102-100000" / "config")
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertLess(page.index("current run"), page.index("20261102-100000"))
        self.assertLess(page.index("20261102-100000"), page.index("20261101-100000"))
        self.assertIn("lowest 11.0", page)

    def test_mod_errors_and_crashes_reported(self):
        self.log(["x"])
        addons = self.pkg / "config" / "addons"
        addons.mkdir()
        (addons / "addons_error.log").write_text("\n".join(f"err {i}" for i in range(9)), encoding="utf-8")
        crashes = self.pkg / "config" / "enhanced-crashes"
        crashes.mkdir()
        (crashes / "crash-1.txt").write_text("boom", encoding="utf-8")
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertIn("(9 lines)", page)
        self.assertIn("err 8", page)
        self.assertNotIn("err 3", page)
        self.assertIn("config/enhanced-crashes/crash-1.txt", page)

    def test_f3_zips_only_newer_than_package(self):
        old = self.desktop / "old-report.zip"
        old.write_bytes(b"z")
        past = time.time() - 3600
        os.utime(self.pkg / "Play X Test.bat", (past, past))
        os.utime(old, (past - 100, past - 100))
        new = self.desktop / "new-report.zip"
        new.write_bytes(b"z")
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertIn("new-report.zip", page)
        self.assertNotIn("old-report.zip", page)

    def test_never_reads_or_prints_account_files_or_absolute_paths(self):
        self.log([r"mod path C:\Users\someone\Documents\thing failed", "ok"])
        (self.pkg / "config" / "account1.opt").write_text("SECRET-PASSWORD", encoding="utf-8")
        (self.pkg / "config" / "acct_main.opt").write_text("SECRET-TWO", encoding="utf-8")
        (self.pkg / "config" / "addons").mkdir()
        (self.pkg / "config" / "addons" / "addons_error.log").write_text(
            f"failed in {self.pkg}\\mods\\x and C:/Users/someone/AppData/y\n", encoding="utf-8"
        )
        before = sorted(p.relative_to(self.pkg).as_posix() for p in self.pkg.rglob("*"))
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertNotIn("SECRET", page)
        self.assertNotIn("account1", page)
        self.assertNotIn("someone", page)
        self.assertNotIn(str(self.pkg), page)
        self.assertNotIn(str(self.base), page)
        self.assertNotIn(":\\", page)
        after = sorted(p.relative_to(self.pkg).as_posix() for p in self.pkg.rglob("*"))
        self.assertEqual(before, after, "the collector writes nothing")

    def test_account_files_are_never_read(self):
        for name in ("account1.opt", "acct_main.opt", "Account.OPT"):
            f = self.pkg / "config" / name
            f.write_text("SECRET", encoding="utf-8")
            self.assertEqual(collect_owner_test.read_lines(f), [], name)

    def test_server_with_credentials_is_not_printed(self):
        (self.pkg / "config" / "server.opt").write_text("user:pw@evil.example\n", encoding="utf-8")
        page = collect_owner_test.collect(self.pkg, self.desktop)
        self.assertIn("server: unknown", page)
        self.assertNotIn("evil.example", page)

    def test_missing_package_is_an_error_without_path(self):
        with mock.patch("sys.stderr", io.StringIO()) as err, self.assertRaises(SystemExit) as cm:
            collect_owner_test.main([str(self.base / "nope"), "--desktop", str(self.desktop)])
        self.assertEqual(cm.exception.code, 1)
        self.assertNotIn(str(self.base), err.getvalue())


if __name__ == "__main__":
    unittest.main()
