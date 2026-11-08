# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Official release preflight and publication regression tests."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import official_release as release

ROOT = Path(__file__).resolve().parents[1]


class OfficialReleaseTests(unittest.TestCase):
    def test_workflow_gives_write_access_only_to_the_publish_job(self):
        workflow = (ROOT / ".github/workflows/official-release.yml").read_text(encoding="utf-8")
        build, publish = workflow.split("\n  publish:", 1)
        self.assertNotIn("secrets.", workflow)
        self.assertNotIn("pull_request", workflow)
        self.assertNotIn("contents: write", build)
        self.assertNotIn("github.token", build)
        self.assertIn("contents: read", build)
        self.assertEqual(workflow.count("contents: write"), 1)
        self.assertIn("contents: write", publish)
        self.assertEqual(workflow.count("GH_TOKEN: ${{ github.token }}"), 1)
        self.assertIn("group: official-release-publication", publish)
        self.assertEqual(workflow.count("persist-credentials: false"), 2)
        self.assertEqual(workflow.count("if: github.repository == 'LoopRook/eqoa-enhanced'"), 2)
        self.assertIn("EQOA_UPDATE_CHANNEL=enhanced", build)
        self.assertIn("cargo +1.99.1 test --locked --workspace", build)

    def test_tag_is_the_canonical_version(self):
        self.assertEqual(release.validate_tag("v0.4.11", "0.4.11"), "0.4.11")
        for tag in ("v0.4.10", "v00.4.11", "HEAD", "v0.4.11;echo bad", "v0.4.11-rc1"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, "0.4.11")

    def test_body_names_the_build_and_carries_the_notes(self):
        body = release.release_body({"version": "0.4.11", "commit": "a" * 40}, "Player notes\n")
        self.assertIn("commit=" + "a" * 40, body)
        self.assertEqual(body.split("-->", 1)[1].strip(), "Player notes")

    def draft(self, body, names=()):
        return {"tag_name": "v0.4.11", "draft": True, "body": body,
                "prerelease": False,
                "assets": [{"name": name} for name in names]}

    def test_draft_requires_exact_build_identity_and_allowed_assets(self):
        body = "identity"
        release.validate_draft(self.draft(body), "0.4.11", body)
        for draft in (self.draft("different build"), self.draft(body, ["extra-notes.md"]),
                      {**self.draft(body), "draft": False},
                      {**self.draft(body), "prerelease": True}):
            with self.subTest(draft=draft), self.assertRaises(ValueError):
                release.validate_draft(draft, "0.4.11", body)

    def test_retry_accepts_matching_bytes_but_refuses_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            local, remote = root / "local", root / "remote"
            local.mkdir()
            remote.mkdir()
            (local / "update.txt").write_bytes(b"expected")
            (remote / "update.txt").write_bytes(b"expected")
            release.validate_existing_assets(local, remote, ["update.txt"])
            (remote / "update.txt").write_bytes(b"another build")
            with self.assertRaises(ValueError):
                release.validate_existing_assets(local, remote, ["update.txt"])

    def test_publish_checks_tag_and_complete_remote_bytes_before_latest(self):
        self.exercise_publish()

    def test_publication_refuses_tag_races_incomplete_or_changed_assets_and_newer_release(self):
        for scenario in ("changed-tag", "missing-assets", "corrupt-download", "newer-release", "different-existing"):
            with self.subTest(scenario=scenario), self.assertRaises(ValueError):
                self.exercise_publish(scenario)

    def exercise_publish(self, scenario=None):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            state = {"version": "0.4.11", "commit": "a" * 40}
            for name in release.asset_names("0.4.11"):
                (output / name).write_bytes(name.encode())
            body = release.release_body(state, "notes")
            calls, uploaded = [], []
            draft = self.draft(body)
            if scenario == "different-existing":
                uploaded.append("update.txt")
                draft = self.draft(body, uploaded)

            def fake_gh(*args):
                calls.append(args)
                if args[0] == "api":
                    if scenario == "newer-release" and uploaded:
                        return json.dumps([[draft, {"tag_name": "v0.4.12", "draft": False}]])
                    return json.dumps([[draft]] if uploaded else [[]])
                if args[:2] == ("release", "upload"):
                    uploaded.append(Path(args[3]).name)
                    draft["assets"] = [{"name": name} for name in uploaded]
                if args[:2] == ("release", "view"):
                    assets = draft["assets"][:-1] if scenario == "missing-assets" else draft["assets"]
                    return json.dumps({"isDraft": True, "isPrerelease": False, "body": body, "assets": assets})
                if args[:2] == ("release", "download"):
                    destination = Path(args[args.index("--dir") + 1])
                    for name in uploaded:
                        data = b"unexpected bytes" if scenario in ("corrupt-download", "different-existing") else (output / name).read_bytes()
                        (destination / name).write_bytes(data)
                return ""

            checks = []
            def tag_check(*args):
                checks.append(True)
                if scenario == "changed-tag" and len(checks) > 1:
                    raise ValueError("Changed tag")
            with patch.object(release, "gh", side_effect=fake_gh), \
                 patch.object(release, "verify_release", return_value={"sourceCommit": "a" * 40, "version": "0.4.11"}), \
                 patch.object(release, "check_tag", side_effect=tag_check):
                try:
                    release.publish(state, output, "notes")
                except ValueError:
                    self.assertFalse(any(call[:2] == ("release", "edit") for call in calls))
                    raise
            self.assertEqual(set(uploaded), release.asset_names("0.4.11"))
            self.assertGreaterEqual(len(checks), 2)
            self.assertEqual(calls[-1][:2], ("release", "edit"))
            self.assertIn("--draft=false", calls[-1])
            self.assertIn("--latest", calls[-1])
            self.assertFalse(any("--clobber" in call for call in calls))

    def test_published_or_newer_release_fails_before_any_mutation(self):
        state = {"version": "0.4.11", "commit": "a" * 40}
        for tag in ("v0.4.11", "v0.4.12"):
            with self.subTest(tag=tag), tempfile.TemporaryDirectory() as directory:
                with patch.object(release, "verify_release", return_value={"sourceCommit": "a" * 40, "version": "0.4.11"}), \
                     patch.object(release, "check_tag"), \
                     patch.object(release, "gh", return_value=json.dumps([[{"tag_name": tag, "draft": False}]])) as api:
                    with self.assertRaises(ValueError):
                        release.publish(state, Path(directory), "notes")
                    self.assertEqual(api.call_count, 1, "history is read before every mutation")

    def test_assets_from_another_build_are_refused_before_any_mutation(self):
        state = {"version": "0.4.11", "commit": "a" * 40}
        with patch.object(release, "verify_release", return_value={"sourceCommit": "b" * 40, "version": "0.4.11"}), \
             patch.object(release, "gh") as api, tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(ValueError):
                release.publish(state, Path(directory), "notes")
            api.assert_not_called()

    def test_tag_check_supports_lightweight_and_annotated_tags_but_rejects_changes(self):
        sha = "b" * 40
        for rows in (f"{sha}\trefs/tags/v0.4.11", f"{'c' * 40}\trefs/tags/v0.4.11\n{sha}\trefs/tags/v0.4.11^{{}}"):
            with patch.object(release, "command", return_value=rows):
                release.check_tag("v0.4.11", sha)
        for rows in ("", f"{'c' * 40}\trefs/tags/v0.4.11"):
            with patch.object(release, "command", return_value=rows), self.assertRaises(ValueError):
                release.check_tag("v0.4.11", sha)

    def git(self, root, *args):
        return release.command("git", "-c", "safe.directory=*", "-C", str(root), *args)

    def make_source(self, directory):
        root = Path(directory) / "source"
        root.mkdir()
        self.git(root, "init", "--quiet")
        self.git(root, "config", "user.name", "Release Test")
        self.git(root, "config", "user.email", "release-test@example.invalid")
        for name, data in {"VERSION": "0.4.11\n", "docs/releases/0.4.11.md": "notes\n"}.items():
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(data, encoding="utf-8")
        self.git(root, "add", ".")
        self.git(root, "commit", "--quiet", "-m", "Fixture")
        self.git(root, "tag", "v0.4.11")
        self.git(root, "update-ref", "refs/remotes/origin/enhanced", "HEAD")
        return root

    def test_actual_preflight_accepts_a_tag_on_enhanced(self):
        with tempfile.TemporaryDirectory() as directory, \
             patch.dict(os.environ, {"GITHUB_REPOSITORY": release.REPO}):
            root = self.make_source(directory)
            state = release.preflight(root, "v0.4.11")
            self.assertEqual(state, {"version": "0.4.11", "commit": self.git(root, "rev-parse", "HEAD")})

    def test_actual_preflight_rejects_moved_unreleased_or_foreign_builds(self):
        for scenario in ("tag-elsewhere", "not-on-enhanced", "wrong-version", "no-notes", "dirty", "fork"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as directory:
                root = self.make_source(directory)
                env = {"GITHUB_REPOSITORY": "someone/fork" if scenario == "fork" else release.REPO}
                tag = "v0.4.12" if scenario == "wrong-version" else "v0.4.11"
                if scenario in ("tag-elsewhere", "not-on-enhanced", "no-notes"):
                    if scenario == "no-notes":
                        self.git(root, "rm", "--quiet", "docs/releases/0.4.11.md")
                        self.git(root, "commit", "--quiet", "-m", "Drop notes")
                        self.git(root, "tag", "--force", "v0.4.11")
                        self.git(root, "update-ref", "refs/remotes/origin/enhanced", "HEAD")
                    else:
                        self.git(root, "commit", "--quiet", "--allow-empty", "-m", "Next commit")
                        if scenario == "not-on-enhanced":
                            self.git(root, "tag", "--force", "v0.4.11")
                if scenario == "dirty":
                    (root / "stray.txt").write_text("x", encoding="utf-8")
                with patch.dict(os.environ, env), self.assertRaises(ValueError):
                    release.preflight(root, tag)


if __name__ == "__main__":
    unittest.main()
