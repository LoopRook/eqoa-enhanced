"""Portable launch contract; never imports or starts the real parser."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import parser_launch

class ParserLaunchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.package = self.base / "mods" / "eqoa_parser"
        self.package.mkdir(parents=True)
        self.parser = self.base / "parser source"
        self.parser.mkdir()
        self.manifest = self.package / "mod.json"
        self.write_manifest(command=sys.executable, args=["-c", "import sys; sys.exit(0)"])
        self.env = patch.dict(os.environ, {"EQOA_PARSER_DIR": str(self.parser)})
        self.env.start()
        self.addCleanup(self.env.stop)

    def write_manifest(self, **changes):
        data = {"id":"eqoa_parser", "enabled":True, "command":"python", "args":["-m", "eqoa_parser", "--client"]}
        data.update(changes)
        self.manifest.write_text(json.dumps(data), encoding="utf-8")

    def test_manifest_arguments_and_configured_working_directory_reach_child(self):
        output = self.base / "observed.json"
        code = "import json,os,sys; from pathlib import Path; Path(sys.argv[1]).write_text(json.dumps([os.getcwd(),sys.argv[2:]]))"
        self.write_manifest(command=sys.executable, args=["-c", code, str(output), 'a "quoted" arg', "back\\slash"])
        child = parser_launch.launch_parser(self.base)
        self.assertEqual(child.wait(timeout=10), 0)
        actual_cwd, actual_args = json.loads(output.read_text())
        self.assertTrue(Path(actual_cwd).samefile(self.parser))
        self.assertFalse(Path(actual_cwd).samefile(self.base))
        self.assertEqual(actual_args, ['a "quoted" arg', "back\\slash"])

    def test_changed_working_directory_does_not_match_previous_directory(self):
        changed_directory = self.base / "different parser directory"
        changed_directory.mkdir()
        os.environ["EQOA_PARSER_DIR"] = str(changed_directory)
        output = self.base / "changed-cwd.json"
        code = "import json,os,sys; from pathlib import Path; Path(sys.argv[1]).write_text(json.dumps(os.getcwd()))"
        self.write_manifest(command=sys.executable, args=["-c", code, str(output)])
        self.assertEqual(parser_launch.launch_parser(self.base).wait(timeout=10), 0)
        actual_cwd = Path(json.loads(output.read_text()))
        self.assertTrue(actual_cwd.samefile(changed_directory))
        self.assertFalse(actual_cwd.samefile(self.parser))

    def test_missing_setting_reports_configuration_instead_of_launching(self):
        os.environ.pop("EQOA_PARSER_DIR", None)
        with self.assertRaisesRegex(parser_launch.ParserLaunchError, "EQOA_PARSER_DIR"):
            parser_launch.launch_parser(self.base)

    def test_missing_manifest_and_disabled_manifest_fail(self):
        self.manifest.unlink()
        with self.assertRaises(parser_launch.ParserLaunchError):
            parser_launch.launch_parser(self.base)
        self.write_manifest(enabled=False)
        with self.assertRaisesRegex(parser_launch.ParserLaunchError, "disabled"):
            parser_launch.launch_parser(self.base)

    def test_malformed_arguments_and_missing_executable_fail(self):
        self.write_manifest(args=[1])
        with self.assertRaisesRegex(parser_launch.ParserLaunchError, "args"):
            parser_launch.launch_parser(self.base)
        self.write_manifest(command="eqoa-deliberately-nonexistent-executable")
        with self.assertRaisesRegex(parser_launch.ParserLaunchError, "command"):
            parser_launch.launch_parser(self.base)

    def test_bare_python_uses_path(self):
        self.write_manifest(command="python", args=["-c", "import sys; sys.exit(0)"])
        self.assertEqual(parser_launch.launch_parser(self.base).wait(timeout=10), 0)

    def test_shipped_manifest_runs_package_main_with_client_flag(self):
        package = self.parser / "eqoa_parser"
        package.mkdir()
        (package / "__init__.py").write_text("", encoding="utf-8")
        (package / "__main__.py").write_text("import json,sys; from pathlib import Path; Path('received.json').write_text(json.dumps(sys.argv[1:]))", encoding="utf-8")
        # This deliberately fails if the obsolete module entry point is selected.
        (package / "app.py").write_text("raise RuntimeError('wrong module')", encoding="utf-8")
        repository = Path(__file__).resolve().parent.parent
        self.assertEqual(parser_launch.launch_parser(repository).wait(timeout=10), 0)
        self.assertEqual(json.loads((self.parser / "received.json").read_text()), ["--client"])

    def test_relative_parser_directory_uses_installation_root(self):
        os.environ["EQOA_PARSER_DIR"] = "parser source"
        self.assertEqual(parser_launch.launch_parser(self.base).wait(timeout=10), 0)
        os.environ["EQOA_PARSER_DIR"] = "missing source"
        with self.assertRaisesRegex(parser_launch.ParserLaunchError, "not a directory"):
            parser_launch.launch_parser(self.base)

    @unittest.skipUnless(os.name == "nt", "Windows process creation contract")
    def test_windows_parser_child_has_no_console_window(self):
        output = self.base / "console.txt"
        code = "import ctypes,sys; from pathlib import Path; Path(sys.argv[1]).write_text(str(ctypes.windll.kernel32.GetConsoleWindow()))"
        self.write_manifest(command=sys.executable, args=["-c", code, str(output)])
        self.assertEqual(parser_launch.launch_parser(self.base).wait(timeout=10), 0)
        self.assertEqual(output.read_text(), "0")

    def test_relative_command_resolves_beside_manifest(self):
        import shutil
        local_python = self.package / Path(sys.executable).name
        shutil.copy2(sys.executable, local_python)
        self.write_manifest(command="./" + local_python.name, args=["-c", "import sys; sys.exit(0)"])
        self.assertEqual(parser_launch.launch_parser(self.base).wait(timeout=10), 0)

if __name__ == "__main__":
    unittest.main()
