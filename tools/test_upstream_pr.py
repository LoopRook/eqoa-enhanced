import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import upstream_pr as up


def run(args, cwd):
    subprocess.run(["git", "-c", "safe.directory=*", *args], cwd=str(cwd), check=True, capture_output=True, text=True)


class Repo:
    """A throwaway repo: branch `master` stands in for origin/master."""

    def __init__(self, tmp):
        self.tmp = Path(tmp)
        self.root = self.tmp / "repo"
        self.root.mkdir()
        run(["init", "-q", "-b", "master"], self.root)
        run(["config", "user.name", "T"], self.root)
        run(["config", "user.email", "t@example.invalid"], self.root)
        self.write("src/lib.rs", "fn a() {\n    let x = 1;\n    let y = 2;\n}\n")
        self.write("src/other.rs", "fn b() {}\n")
        run(["add", "."], self.root)
        run(["commit", "-q", "-m", "base"], self.root)
        # a source branch holding the change to be offered
        run(["checkout", "-q", "-b", "src-change"], self.root)

    def write(self, rel, text):
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8", newline="\n")

    def commit_source(self):
        run(["add", "."], self.root)
        run(["commit", "-q", "-m", "work"], self.root)
        run(["checkout", "-q", "master"], self.root)

    def spec(self, **kw):
        s = {"message": "fix: thing", "allowed_files": ["src/lib.rs"],
             "checkout": {"ref": "src-change", "paths": ["src/lib.rs"]}}
        s.update(kw)
        p = self.tmp / "spec.json"
        p.write_text(json.dumps(s), encoding="utf-8")
        return p

    def cut(self, spec, name="t1"):
        return up.cut(self.root, name, spec, base="master", worktree=self.tmp / "wt")


GOOD = "fn a() {\n    let x = 1;\n    let y = 2;\n    let z = guard(y);\n}\n"


class CheckDiffTests(unittest.TestCase):
    def diff(self, files):
        """files: {path: (old, new)} -> a -U0 diff via a scratch repo."""
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            run(["checkout", "-q", "master"], r.root)
            for p, (old, new) in files.items():
                r.write(p, old)
            run(["add", "."], r.root)
            run(["commit", "-q", "-m", "o"], r.root)
            for p, (old, new) in files.items():
                r.write(p, new)
            return up.git(["diff", "-U0"], r.root).stdout

    def test_clean_diff_passes_and_planted_fork_name_fails(self):
        clean = self.diff({"a.rs": ("x\n", "x\ny = guard();\n")})
        self.assertEqual(up.check_diff(clean, ["a.rs"]), [])
        for name in ["fork_hook()", "forge_runtime", "// Enhanced only", "LoopRook", "eqoa_enhanced"]:
            bad = self.diff({"a.rs": ("x\n", f"x\n{name}\n")})
            errs = up.check_diff(bad, ["a.rs"])
            self.assertEqual(len(errs), 1, name)
            self.assertTrue(errs[0].startswith("a.rs:2:"), errs[0])

    def test_fork_name_in_removed_line_is_fine(self):
        d = self.diff({"a.rs": ("fork_old()\nkeep\n", "keep\n")})
        self.assertEqual(up.check_diff(d, ["a.rs"]), [])

    def test_allow_names(self):
        d = self.diff({"a.rs": ("x\n", "x\nlet lua = mlua::Lua::new();\n")})
        self.assertTrue(up.check_diff(d, ["a.rs"]))
        self.assertEqual(up.check_diff(d, ["a.rs"], ["lua", "mlua"]), [])

    def test_game_data_paths(self):
        for p in ["run/eqoa.iso", "Testing Resources/x.txt", "disc.iso", "mods/decrypted_module.rs", "keys/a.pem"]:
            d = self.diff({p: ("a\n", "b\n")})
            errs = up.check_diff(d, [p])
            self.assertTrue(any("refused" in e and e.startswith(p) for e in errs), (p, errs))

    def test_whitespace_only_and_reorder_hunks(self):
        d = self.diff({"a.rs": ("let x = 1;\nlet y = 2;\n", "let x  =  1;\nlet y = 2;\n")})
        errs = up.check_diff(d, ["a.rs"])
        self.assertEqual(len(errs), 1)
        self.assertIn("a.rs:1:", errs[0])
        self.assertIn("formatting-only", errs[0])
        d = self.diff({"a.rs": ("one\ntwo\n", "two\none\n")})
        self.assertIn("formatting-only", "".join(up.check_diff(d, ["a.rs"])))
        # a real change on the same line is not churn
        d = self.diff({"a.rs": ("let x = 1;\n", "let x = 2;\n")})
        self.assertEqual(up.check_diff(d, ["a.rs"]), [])

    def test_unnamed_file(self):
        d = self.diff({"a.rs": ("a\n", "b\n"), "b.rs": ("a\n", "b\n")})
        errs = up.check_diff(d, ["a.rs"])
        self.assertEqual(errs, ["b.rs: refused, not named by the spec"])
        self.assertEqual(up.check_diff(d, ["*.rs"]), [])


class BranchNameTests(unittest.TestCase):
    def test_prefix_added_and_others_refused(self):
        self.assertEqual(up.branch_name("fix-pad"), "upstream-pr/fix-pad")
        self.assertEqual(up.branch_name("upstream-pr/x"), "upstream-pr/x")
        for bad in ["develop/x", "feature/x", "master", "../x", "-x", "", "upstream-pr/", "a b", "upstream-pr/../x"]:
            if bad == "master":
                continue  # a plain name is prefixed, so it can never be master
            with self.assertRaises(up.Refusal, msg=bad):
                up.branch_name(bad)
        with self.assertRaises(up.Refusal):
            up.require_pr_branch("develop")

    def test_cli_refuses_non_pr_branch_without_touching_repo(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            run(["checkout", "-q", "master"], r.root)
            before = up.git(["branch", "--list"], r.root).stdout
            rc = up.main(["--repo", str(r.root), "check", "develop/x"])
            self.assertEqual(rc, 2)
            rc = up.main(["--repo", str(r.root), "cut", "feature/x", "--spec", str(r.spec()), "--base", "master"])
            self.assertEqual(rc, 2)
            self.assertEqual(up.git(["branch", "--list"], r.root).stdout, before)

    def test_no_push_option(self):
        with self.assertRaises(SystemExit):
            up.main(["cut", "x", "--spec", "s", "--push"])


class CutTests(unittest.TestCase):
    def test_cut_commits_only_the_named_change_and_recut_resets(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            r.write("src/lib.rs", GOOD)
            r.commit_source()
            wt = r.cut(r.spec())
            self.assertEqual(up.git(["rev-parse", "--abbrev-ref", "HEAD"], wt).stdout.strip(), "upstream-pr/t1")
            self.assertEqual((wt / "src/lib.rs").read_text(encoding="utf-8"), GOOD)
            self.assertEqual(up.git(["log", "--format=%s", "master..HEAD"], wt).stdout.split(), ["fix:", "thing"])
            self.assertEqual(up.check(r.root, "t1", base="master"), [])
            # the main checkout was never touched
            self.assertEqual(up.git(["rev-parse", "--abbrev-ref", "HEAD"], r.root).stdout.strip(), "master")
            # re-cut resets to a single commit
            (wt / "scratch.txt").write_text("x", encoding="utf-8")
            wt2 = r.cut(r.spec())
            self.assertEqual(wt2, wt)
            self.assertFalse((wt / "scratch.txt").exists())
            self.assertEqual(len(up.git(["rev-list", "master..HEAD"], wt).stdout.split()), 1)

    def test_cut_refuses_fork_name_and_leaves_branch_at_base(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            r.write("src/lib.rs", GOOD.replace("guard", "fork_guard"))
            r.commit_source()
            with self.assertRaises(up.Refusal) as cm:
                r.cut(r.spec())
            self.assertIn("src/lib.rs:4", str(cm.exception))
            self.assertIn("fork_", str(cm.exception))
            base = up.git(["rev-parse", "master"], r.root).stdout
            self.assertEqual(up.git(["rev-parse", "upstream-pr/t1"], r.root).stdout, base)
            # negative control: the same change without the name is accepted
            r.write("src/lib.rs", GOOD)
            run(["checkout", "-q", "-b", "src-good"], r.root)
            r.write("src/lib.rs", GOOD)
            r.commit_source()
            r.cut(r.spec(checkout={"ref": "src-good", "paths": ["src/lib.rs"]}))
            self.assertEqual(up.check(r.root, "t1", base="master"), [])

    def test_cut_refuses_unnamed_iso_run_and_whitespace(self):
        for rel, text, spec_files in [
            ("src/other.rs", "fn b() { 1 }\n", ["src/lib.rs"]),          # file not named
            ("run/eqoa.iso", "x", ["src/lib.rs", "run/eqoa.iso"]),        # game data, even if named
            ("src/lib.rs", "fn a() {\n  let x = 1;\n    let y = 2;\n}\n", ["src/lib.rs"]),  # whitespace only
        ]:
            with tempfile.TemporaryDirectory() as d:
                r = Repo(d)
                r.write(rel, text)
                r.commit_source()
                spec = r.spec(allowed_files=spec_files,
                              checkout={"ref": "src-change", "paths": sorted({rel, "src/lib.rs"} & {rel} | {rel})})
                with self.assertRaises(up.Refusal, msg=rel) as cm:
                    r.cut(spec)
                self.assertIn(rel, str(cm.exception))

    def test_worktree_inside_repo_refused(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            r.write("src/lib.rs", GOOD)
            r.commit_source()
            with self.assertRaises(up.Refusal):
                up.cut(r.root, "t1", r.spec(), base="master", worktree=r.root / "inside")
            with self.assertRaises(up.Refusal):
                up.cut(r.root, "t1", r.spec(), base="master", worktree=r.root)

    def test_patch_spec(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            r.write("src/lib.rs", GOOD)
            run(["add", "."], r.root)
            patch = up.git(["diff", "--cached", "master"], r.root).stdout if False else None
            run(["commit", "-q", "-am", "work"], r.root)
            patch = up.git(["diff", "master", "src-change"], r.root).stdout
            run(["checkout", "-q", "master"], r.root)
            (r.tmp / "p.patch").write_text(patch, encoding="utf-8", newline="\n")
            spec = r.spec(checkout=None, patches=["p.patch"])
            del spec  # the helper writes the file; rebuild without checkout
            p = r.tmp / "spec2.json"
            p.write_text(json.dumps({"message": "m", "allowed_files": ["src/lib.rs"], "patches": ["p.patch"]}),
                         encoding="utf-8")
            wt = r.cut(p)
            self.assertEqual((wt / "src/lib.rs").read_text(encoding="utf-8"), GOOD)


class TestCommandTests(unittest.TestCase):
    def test_print_only_and_target_inside_worktree_refused(self):
        with tempfile.TemporaryDirectory() as d:
            r = Repo(d)
            r.write("src/lib.rs", GOOD)
            r.commit_source()
            wt = r.cut(r.spec())
            import os
            old = os.environ.get("CARGO_TARGET_DIR")
            try:
                os.environ["CARGO_TARGET_DIR"] = str(wt / "target")
                with self.assertRaises(up.Refusal):
                    up.test(r.root, "t1", print_only=True)
                os.environ["CARGO_TARGET_DIR"] = str(r.tmp / "shared")
                self.assertEqual(up.test(r.root, "t1", print_only=True), 0)
            finally:
                if old is None:
                    os.environ.pop("CARGO_TARGET_DIR", None)
                else:
                    os.environ["CARGO_TARGET_DIR"] = old


if __name__ == "__main__":
    unittest.main()
