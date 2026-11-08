# LoopRook's additions in this file are under the GNU Affero General Public License v3 or later (see LICENSE).
"""Exercise the upstream sync script: its struct-literal merge, and a whole sync in disposable repositories."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
import sync_upstream as sync  # noqa: E402

REPO = TOOLS.parent


def git(root, *args, **kw):
    return subprocess.run(["git", "-c", "safe.directory=*", "-C", str(root), *args],
                          capture_output=True, text=True, check=True, **kw).stdout


class StructLiteralMerge(unittest.TestCase):
    BASE = "let mut p = Play { session, a: 1, b: Default::default(), duel: Default::default() };"

    def test_both_sides_add_fields(self):
        ours = "let mut p = Play { session, a: 1, fork: Fork::new(), b: Default::default(), duel: Default::default() };"
        theirs = "let mut p = Play { session, a: 1, b: Default::default(), duel: Default::default(), record: Default::default() };"
        self.assertEqual(sync.merge_struct_literal(self.BASE, ours, theirs),
                         "let mut p = Play { session, a: 1, fork: Fork::new(), b: Default::default(), "
                         "duel: Default::default(), record: Default::default() };")

    def test_upstream_removal_and_one_sided_change(self):
        ours = "let mut p = Play { session, a: 2, b: Default::default(), duel: Default::default() };"
        theirs = "let mut p = Play { session, a: 1, duel: Default::default(), c: x };"
        self.assertEqual(sync.merge_struct_literal(self.BASE, ours, theirs),
                         "let mut p = Play { session, a: 2, duel: Default::default(), c: x };")

    def test_nested_braces_strings_and_attributes(self):
        base = 'X { o: G { c: 1, ..Default::default() }, s: "a, }", ch: \'{\', d: 0 };'
        ours = 'X { o: G { c: 1, ..Default::default() }, #[cfg(not(target_arch = "wasm32"))] h: H::default(), s: "a, }", ch: \'{\', d: 0 };'
        theirs = 'X { o: G { c: 2, ..Default::default() }, s: "a, }", ch: \'{\', d: 0, e: 1 };'
        self.assertEqual(sync.merge_struct_literal(base, ours, theirs),
                         'X { o: G { c: 2, ..Default::default() }, #[cfg(not(target_arch = "wasm32"))] h: H::default(), '
                         's: "a, }", ch: \'{\', d: 0, e: 1 };')

    def test_real_conflicts_are_left_for_a_person(self):
        # The same field changed differently on both sides.
        self.assertIsNone(sync.merge_struct_literal(
            self.BASE, self.BASE.replace("a: 1", "a: 2"), self.BASE.replace("a: 1", "a: 3")))
        # Ours changed a field upstream removed.
        self.assertIsNone(sync.merge_struct_literal(
            self.BASE, self.BASE.replace("a: 1", "a: 2"), self.BASE.replace("a: 1, ", "")))
        # Not a struct literal.
        self.assertIsNone(sync.merge_struct_literal("foo(a, b);", "foo(a, b, c);", "foo(a, d, b);"))
        # The struct renamed differently on each side.
        self.assertIsNone(sync.merge_struct_literal(self.BASE, self.BASE.replace("Play", "Game"), self.BASE.replace("Play", "Thing")))

    def test_resolve_text_merges_only_struct_literal_hunks(self):
        text = ("fn a() {\r\n<<<<<<< ours\r\n" + self.BASE.replace("a: 1", "a: 1, f: 0") + "\r\n||||||| base\r\n"
                + self.BASE + "\r\n=======\r\n" + self.BASE.replace("duel", "duel: 1, r") + "\r\n>>>>>>> theirs\r\n}\r\n"
                "<<<<<<< ours\r\nx();\r\n||||||| base\r\ny();\r\n=======\r\nz();\r\n>>>>>>> theirs\r\n")
        new, done, left = sync.resolve_text(text)
        self.assertEqual((done, left), (1, 1))
        self.assertIn("let mut p = Play { session, a: 1, f: 0, b: Default::default(), duel: 1, r: Default::default() };\r\n}\r\n", new)
        self.assertIn("<<<<<<< ours\r\nx();", new, "the control: an ordinary conflict stays")

    def test_replays_the_2026_11_08_play_rs_conflict(self):
        """The real hunk from sync/2026-11-08 resolves to what was committed by hand (d59c0ae)."""
        refs = {"base": "eb5916bfd25053dcc047c3a050a2529577842306", "ours": "429431c", "theirs": "72a7d48", "merged": "d59c0ae"}
        try:
            lines = {k: next(line for line in git(REPO, "show", f"{r}:crates/eqoa/src/play.rs").splitlines()
                             if line.lstrip().startswith("let mut p = Play {"))
                     for k, r in refs.items()}
        except (subprocess.CalledProcessError, StopIteration):
            self.skipTest("SKIP: the 2026-11-08 sync commits are not in this clone")
        self.assertNotEqual(lines["ours"], lines["theirs"])
        self.assertEqual(sync.merge_struct_literal(lines["base"], lines["ours"], lines["theirs"]), lines["merged"])


class WholeSync(unittest.TestCase):
    """A fork and its upstream in a temp folder; the script syncs them with --no-tests."""

    LIT = "let p = Play {{ session, a: 1{fork}, duel: 0{up} }};\n"

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.top = Path(self.temp.name)
        self.up = self.top / "upstream"
        self.fork = self.top / "fork"
        self.up.mkdir()
        self.init(self.up)
        self.write(self.up, "CLAUDE.md", "upstream rules\n")
        self.write(self.up, "README.md", "# vanilla\n")
        self.write(self.up, "play.rs", "fn new() {\n" + self.LIT.format(fork="", up="") + "}\n")
        self.write(self.up, "other.rs", "fn x() { 1 }\n")
        git(self.up, "add", ".")
        git(self.up, "commit", "-qm", "upstream base")
        base = git(self.up, "rev-parse", "HEAD").strip()
        subprocess.run(["git", "-c", "core.autocrlf=false", "clone", "-q", "-c", "core.autocrlf=false", str(self.up), str(self.fork)], check=True)
        self.init(self.fork)
        git(self.fork, "checkout", "-q", "-b", "develop")
        shutil.copy(REPO / ".gitattributes", self.fork / ".gitattributes")  # the real merge rules
        self.write(self.fork, ".gitignore", "/target\n")
        self.write(self.fork, "CLAUDE.md", "fork rules\n")
        self.write(self.fork, "README.md", "# enhanced\n")
        self.write(self.fork, "docs/upstream/CLAUDE.md", "upstream rules\n")
        self.write(self.fork, "play.rs", "fn new() {\n" + self.LIT.format(fork=", fork: F::new()", up="") + "}\n")
        (self.fork / "tools").mkdir()
        shutil.copy(TOOLS / "check_fork_footprint.py", self.fork / "tools")
        self.manifest = {
            "schema_version": 1, "reviewed_base": base,
            "fork_owned": [".gitattributes", ".gitignore", "README.md", "docs/**", "tools/**"],
            "shared": {"play.rs": {"added": 1, "deleted": 1, "reason": "fork field"}},
            "immutable": [], "claude_upstream_copy": "docs/upstream/CLAUDE.md", "license_manifests": [], "hooks": [],
        }
        self.write(self.fork, "docs/fork-footprint.json", json.dumps(self.manifest, indent=2) + "\n")
        git(self.fork, "add", ".")
        git(self.fork, "commit", "-qm", "fork")

    def init(self, root):
        if not (root / ".git").exists():
            git(root, "init", "-q", "-b", "master")
        git(root, "config", "user.email", "sync@example.invalid")
        git(root, "config", "user.name", "Sync test")
        git(root, "config", "core.autocrlf", "false")

    def write(self, root, rel, text):
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")

    def upstream_commit(self, files):
        for rel, text in files.items():
            self.write(self.up, rel, text)
        git(self.up, "add", ".")
        git(self.up, "commit", "-qm", "upstream feature")

    def run_sync(self, *args, cwd=None):
        return subprocess.run([sys.executable, str(TOOLS / "sync_upstream.py"), "--date", "2026-01-01", "--no-tests", *args],
                              cwd=cwd or self.fork, capture_output=True, text=True, encoding="utf-8")

    def test_routine_conflicts_resolve_and_the_merge_is_committed(self):
        self.upstream_commit({"README.md": "# vanilla, more\n", "CLAUDE.md": "upstream rules, more\n",
                              "play.rs": "fn new() {\n" + self.LIT.format(fork="", up=", record: 0") + "}\n"})
        r = self.run_sync()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        wt = self.top / "eqoa-sync-2026-01-01"
        self.assertEqual((wt / "README.md").read_text(), "# enhanced\n", "README keeps ours")
        self.assertEqual((wt / "CLAUDE.md").read_text(), "fork rules\n")
        self.assertEqual((wt / "docs/upstream/CLAUDE.md").read_text(), "upstream rules, more\n")
        self.assertIn("let p = Play { session, a: 1, fork: F::new(), duel: 0, record: 0 };", (wt / "play.rs").read_text())
        self.assertEqual(json.loads((wt / "docs/fork-footprint.json").read_text())["reviewed_base"],
                         git(self.fork, "rev-parse", "origin/master").strip())
        self.assertEqual(git(wt, "status", "--porcelain"), "")
        self.assertEqual(len(git(wt, "log", "-1", "--format=%P").split()), 2, "a real two-parent merge")
        self.assertIn("merged field by field", (wt / "target/sync-report.md").read_text())

    def test_a_real_conflict_stops_then_continue_finishes(self):
        self.write(self.fork, "other.rs", "fn x() { 2 }\n")
        git(self.fork, "commit", "-qam", "fork edits other.rs")
        self.manifest["shared"]["other.rs"] = {"added": 1, "deleted": 1, "reason": "fork change"}
        self.upstream_commit({"other.rs": "fn x() { 3 }\n"})
        r = self.run_sync()
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)
        self.assertIn("other.rs (1 hunk(s))", r.stdout)
        wt = self.top / "eqoa-sync-2026-01-01"
        self.write(wt, "other.rs", "fn x() { 2 + 3 }\n")
        self.write(wt, "docs/fork-footprint.json", json.dumps(self.manifest, indent=2) + "\n")
        git(wt, "add", ".")
        r = self.run_sync("--continue", cwd=wt)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(len(git(wt, "log", "-1", "--format=%P").split()), 2)
        self.assertEqual(git(wt, "status", "--porcelain"), "")

    def test_a_budget_overrun_stops_before_the_commit(self):
        self.write(self.fork, "other.rs", "fn x() { 1 }\nfn fork() {}\n")
        git(self.fork, "commit", "-qam", "fork adds to other.rs without a budget")
        self.upstream_commit({"CLAUDE.md": "upstream rules, more\n"})
        r = self.run_sync()
        self.assertEqual(r.returncode, 3, r.stdout + r.stderr)
        self.assertIn("unreviewed shared path: other.rs", r.stdout)
        wt = self.top / "eqoa-sync-2026-01-01"
        self.assertTrue((Path(git(wt, "rev-parse", "--absolute-git-dir").strip()) / "MERGE_HEAD").exists(),
                        "not committed")

    def test_nothing_to_sync(self):
        r = self.run_sync()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("Nothing to sync", r.stdout)
        self.assertFalse((self.top / "eqoa-sync-2026-01-01").exists())


class RepositorySetup(unittest.TestCase):
    def test_readme_and_claude_keep_ours(self):
        attrs = (REPO / ".gitattributes").read_text(encoding="utf-8").splitlines()
        self.assertIn("CLAUDE.md merge=ours", attrs)
        self.assertIn("README.md merge=ours", attrs)


if __name__ == "__main__":
    unittest.main()
