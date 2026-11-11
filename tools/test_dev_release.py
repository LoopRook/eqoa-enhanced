# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Rolling development release preflight and publication regression tests."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import dev_release as release
from check_release_assets import check_history

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "a" * 40
OTHER = "b" * 40
VERSION = "0.4.14"
OLD_ZIP = "EQOA-Enhanced-0.4.13-Windows-x64.zip"


class DevelopmentReleaseTests(unittest.TestCase):
    def test_workflow_builds_develop_and_only_publish_may_write(self):
        workflow = (ROOT / ".github/workflows/enhanced-dev.yml").read_text(encoding="utf-8")
        build, publish = workflow.split("\n  publish:", 1)
        self.assertIn("branches: [develop]", workflow)
        ignored = workflow.split("paths-ignore:", 1)[1].split("workflow_dispatch:", 1)[0]
        self.assertEqual([line.strip() for line in ignored.splitlines() if line.strip().startswith("-")],
                         ["- '*.md'", "- 'docs/**'", "- '.github/**'"],
                         "only documentation skips a nightly; bundled mods and code never do")
        self.assertIn("workflow_dispatch:", workflow)
        self.assertNotIn("pull_request", workflow)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("contents: write", build)
        self.assertNotIn("github.token", build)
        self.assertIn("contents: read", build)
        self.assertEqual(workflow.count("contents: write"), 1)
        self.assertIn("contents: write", publish)
        self.assertEqual(workflow.count("GH_TOKEN: ${{ github.token }}"), 1)
        self.assertEqual(workflow.count("if: github.repository == 'LoopRook/eqoa-enhanced'"), 2)
        self.assertIn("cancel-in-progress: true", workflow)
        self.assertIn("EQOA_UPDATE_CHANNEL=enhanced-dev", build)
        self.assertIn("EQOA_BUILD_COMMIT=", build)
        self.assertIn("tools/dev_release.py preflight", build)
        self.assertIn("--channel enhanced-dev", build)
        self.assertIn("tools/dev_release.py publish --assets", publish)

    def test_nightly_names_its_zip_and_shows_only_unreleased_notes(self):
        self.assertIn("EQOA-Enhanced-Nightly-Windows-x64.zip", release.asset_names(VERSION))
        self.assertNotIn(f"EQOA-Enhanced-{VERSION}-Windows-x64.zip", release.asset_names(VERSION))
        notes = "# Notes\n\n## Unreleased\n\n- New thing.\n\n## 2026-11-08 · Enhanced 0.4.14\n\n- Old thing.\n"
        body = release.release_body(COMMIT, notes)
        self.assertIn("- New thing.", body)
        self.assertNotIn("Old thing", body)
        self.assertIn("Same as Stable.", release.release_body(COMMIT, "# Notes\n\n## 2026-11-08 · Enhanced 0.4.14\n\n- Old thing.\n"))

    def test_real_notes_publish_the_unreleased_bullets(self):
        # The real notes either carry an Unreleased section (between releases:
        # Nightly publishes exactly it) or none (a release renames it in place:
        # Nightly says it is the same as Stable). Never a dated release entry.
        notes = (ROOT / "FORK-UPDATE-NOTES.md").read_text(encoding="utf-8")
        body = release.release_body(COMMIT, notes)
        section = release.unreleased(notes)
        self.assertNotIn("· Enhanced", body)
        if section:
            self.assertNotIn("Same as Stable.", body)
            self.assertIn(section, body)
            renamed = notes.replace("## Unreleased", "## 2026-11-09 · Enhanced dev build", 1)
            self.assertIn("Same as Stable.", release.release_body(COMMIT, renamed),
                          "a renamed heading no longer publishes the Nightly notes")
        else:
            self.assertIn("Same as Stable.", body)
            with_section = notes.replace("\n## ", "\n## Unreleased\n\n- Next thing.\n\n## ", 1)
            self.assertIn("- Next thing.", release.release_body(COMMIT, with_section),
                          "an Unreleased section added after the release is published")

    def test_unreleased_heading_must_keep_the_published_format(self):
        for heading in ("## Unreleased Notes", "### Unreleased", "## unreleased", "## Unreleased "):
            with self.subTest(heading=heading):
                self.assertIsNotNone(release.unreleased_heading_problem(f"# Notes\n\n{heading}\n\n- a\n"))
        self.assertIsNone(release.unreleased_heading_problem("# Notes\n\n## Unreleased\n\n- a\n"))
        self.assertIsNotNone(release.unreleased_heading_problem("## Unreleased\n- a\n\n## Unreleased\n- b\n"))

    def test_history_accepts_the_rolling_dev_tag_beside_versioned_releases(self):
        check_history([{"tagName": "dev", "isDraft": False},
                       {"tagName": "v0.4.13", "isDraft": False}], VERSION)
        with self.assertRaises(ValueError):
            check_history([{"tagName": "v0.4.14", "isDraft": False}], VERSION)

    def write_assets(self, output, version=VERSION):
        for name in release.asset_names(version):
            (output / name).write_bytes(f"{version}:{name}".encode())

    def manifest(self, commit=COMMIT, version=VERSION):
        return {"sourceCommit": commit, "version": version}

    def exercise_publish(self, scenario="stale"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "assets"
            output.mkdir()
            self.write_assets(output)
            calls, uploads = [], []
            release_body = release.release_body(COMMIT, "notes")
            state = {"exists": scenario != "create", "body": release_body,
                     "prerelease": True, "latest": False, "draft": False,
                     # the first nightly's ZIP carried the version in its name
                     "assets": (sorted(release.asset_names("0.4.13") | {OLD_ZIP})
                                if scenario == "stale" else [])}

            def fake_gh(*args):
                args = tuple(map(str, args))
                calls.append(args)
                if args[:3] == ("api", "--paginate", "--slurp"):
                    return json.dumps([[{"tag_name": "dev", "draft": False,
                                         "prerelease": state["prerelease"],
                                         "assets": [{"name": name} for name in state["assets"]]}]
                                       if state["exists"] else []])
                if args[:2] == ("api", f"repos/{release.REPO}/releases/latest"):
                    return "dev" if state["latest"] else "v0.4.14"
                if args[0] == "api":
                    return ""
                if args[:2] == ("release", "create"):
                    state["exists"] = True
                    state["assets"] = []
                    state["body"] = Path(args[args.index("--notes-file") + 1]).read_text(encoding="utf-8")
                elif args[:2] == ("release", "upload"):
                    name = Path(args[3]).name
                    uploads.append(name)
                    if name not in state["assets"]:
                        state["assets"].append(name)
                elif args[:2] == ("release", "delete-asset"):
                    state["assets"].remove(args[3])
                elif args[:2] == ("release", "edit"):
                    state["body"] = Path(args[args.index("--notes-file") + 1]).read_text(encoding="utf-8")
                elif args[:2] == ("release", "view"):
                    # the real `gh release view` has no isLatest field
                    assert "isLatest" not in args[args.index("--json") + 1]
                    return json.dumps({"isDraft": state["draft"], "isPrerelease": state["prerelease"],
                                       "body": state["body"],
                                       "assets": [{"name": name} for name in state["assets"]]})
                elif args[:2] == ("release", "download"):
                    destination = Path(args[args.index("--dir") + 1])
                    for name in state["assets"]:
                        (destination / name).write_bytes((output / name).read_bytes())
                return ""

            def fake_command(*args, cwd=None):
                ref = args[-1]
                if ref == "refs/heads/develop":
                    return f"{COMMIT}\trefs/heads/develop"
                if ref.endswith("refs/tags/dev"):
                    return f"{COMMIT}\trefs/tags/dev"
                raise AssertionError(args)

            with patch.object(release, "gh", side_effect=fake_gh), \
                 patch.object(release, "command", side_effect=fake_command), \
                 patch.object(release, "verify_release", return_value=self.manifest()):
                result = release.publish(COMMIT, output, "notes")
            return result, calls, uploads, state

    def test_publish_uploads_the_manifest_last_and_removes_stale_assets(self):
        result, calls, uploads, state = self.exercise_publish()
        self.assertEqual(result, "published")
        self.assertEqual(uploads[-1], "update.txt")
        self.assertEqual(set(uploads), release.asset_names(VERSION))
        self.assertNotIn(OLD_ZIP, state["assets"])
        self.assertEqual(set(state["assets"]), release.asset_names(VERSION))
        self.assertTrue(any(call[:2] == ("release", "delete-asset") and call[3] == OLD_ZIP for call in calls))

    def test_publish_creates_the_rolling_release_when_missing(self):
        result, calls, uploads, state = self.exercise_publish("create")
        self.assertEqual(result, "published")
        self.assertEqual(uploads[-1], "update.txt")
        self.assertEqual(state["assets"], sorted(release.asset_names(VERSION)))
        creates = [call for call in calls if call[:2] == ("release", "create")]
        self.assertEqual(len(creates), 1)
        self.assertIn("--verify-tag", creates[0])
        self.assertIn(release.TITLE, creates[0])
        self.assertTrue(any(call[:2] == ("api", "-X") for call in calls), "the rolling tag moves")

    def test_never_makes_the_development_release_latest_or_stable(self):
        for scenario in ("stale", "create"):
            with self.subTest(scenario=scenario):
                _, calls, _, _ = self.exercise_publish(scenario)
                for call in calls:
                    if "--latest" in call:
                        self.assertIn("--latest=false", call)
                    if call[:2] in (("release", "create"), ("release", "edit")):
                        self.assertIn("--prerelease", call)
                        self.assertNotIn("--prerelease=false", call)

    def test_superseded_develop_publishes_nothing(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(release, "verify_release", return_value=self.manifest()), \
             patch.object(release, "remote_develop", return_value=OTHER), \
             patch.object(release, "gh") as api:
            result = release.publish(COMMIT, Path(directory), "notes")
        self.assertEqual(result, "superseded")
        api.assert_not_called()

    def test_manifest_commit_mismatch_is_refused_before_any_remote_call(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.object(release, "verify_release", return_value=self.manifest(OTHER)), \
             patch.object(release, "command") as git, \
             patch.object(release, "gh") as api:
            with self.assertRaises(ValueError):
                release.publish(COMMIT, Path(directory), "notes")
        git.assert_not_called()
        api.assert_not_called()

    def git(self, root, *args):
        return release.command("git", "-c", "safe.directory=*", "-C", str(root), *args)

    def make_source(self, directory):
        root = Path(directory) / "source"
        root.mkdir()
        self.git(root, "init", "--quiet")
        self.git(root, "config", "user.name", "Release Test")
        self.git(root, "config", "user.email", "release-test@example.invalid")
        (root / "VERSION").write_text("0.4.14\n", encoding="utf-8")
        (root / "FORK-UPDATE-NOTES.md").write_text("notes\n", encoding="utf-8")
        self.git(root, "add", ".")
        self.git(root, "commit", "--quiet", "-m", "Fixture")
        self.git(root, "update-ref", "refs/remotes/origin/develop", "HEAD")
        return root

    def test_actual_preflight_accepts_a_commit_on_develop(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {"GITHUB_REPOSITORY": release.REPO}):
            root = self.make_source(directory)
            state = release.preflight(root)
            self.assertEqual(state, {"commit": self.git(root, "rev-parse", "HEAD")})

    def test_actual_preflight_refuses_a_malformed_unreleased_heading(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {"GITHUB_REPOSITORY": release.REPO}):
            root = self.make_source(directory)
            (root / "FORK-UPDATE-NOTES.md").write_text("## Unreleased notes\n- a\n", encoding="utf-8")
            self.git(root, "commit", "--quiet", "-am", "Malformed notes")
            self.git(root, "update-ref", "refs/remotes/origin/develop", "HEAD")
            with self.assertRaises(ValueError):
                release.preflight(root)

    def test_actual_preflight_refuses_commits_not_on_develop(self):
        for scenario in ("not-on-develop", "dirty", "fork", "missing-develop"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as directory:
                root = self.make_source(directory)
                env = {"GITHUB_REPOSITORY": "someone/fork" if scenario == "fork" else release.REPO}
                if scenario == "not-on-develop":
                    self.git(root, "commit", "--quiet", "--allow-empty", "-m", "Next commit")
                elif scenario == "dirty":
                    (root / "stray.txt").write_text("x", encoding="utf-8")
                elif scenario == "missing-develop":
                    self.git(root, "update-ref", "-d", "refs/remotes/origin/develop")
                with patch.dict(os.environ, env), self.assertRaises(ValueError):
                    release.preflight(root)


if __name__ == "__main__":
    unittest.main()
