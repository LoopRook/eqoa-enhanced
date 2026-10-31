"""Offline release contract tests; run with python -m unittest discover -s tools -p test_player_release.py."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

MODULE_PATH = Path(__file__).with_name("build_player_release.py")

BUNDLED_MOD_FILES = [
    "resources/fonts/Cinzel/OFL.txt",
    "resources/fonts/Cinzel/static/Cinzel-Regular.ttf",
    "resources/fonts/Libre_Baskerville/OFL.txt",
    "resources/fonts/Libre_Baskerville/static/LibreBaskerville-Regular.ttf",
    "resources/fonts/Merriweather_Sans/OFL.txt",
    "resources/fonts/Merriweather_Sans/static/MerriweatherSans-Regular.ttf",
    "resources/gear-icon.png",
    "wereoxxs_ui/README.md",
    "wereoxxs_ui/modules/ability_bars.lua",
    "wereoxxs_ui/modules/casting_bar.lua",
    "wereoxxs_ui/modules/chat_window.lua",
    "wereoxxs_ui/modules/class_art.lua",
    "wereoxxs_ui/modules/class_icon.lua",
    "wereoxxs_ui/modules/damage_meter.lua",
    "wereoxxs_ui/modules/meter.lua",
    "wereoxxs_ui/modules/paint.lua",
    "wereoxxs_ui/modules/party_frames.lua",
    "wereoxxs_ui/modules/solo_meter.lua",
    "wereoxxs_ui/modules/tape_compass.lua",
    "wereoxxs_ui/wereoxxs_ui.lua",
]

EXPECTED_ZIP_MEMBERS = [
    "eqoa.exe", "map-entities.json", "LICENSE", "FrontiersForge-MIT.txt",
    "SpaceMono-OFL.txt", "Native-Addon-Runtime-NOTICES.txt", "README.txt",
    "release-manifest.json", "mods/README.txt",
    *(f"mods/{rel}" for rel in BUNDLED_MOD_FILES),
]


def minimal_pe(commit, download_base, machine=0x8664, subsystem=2, stack=8 * 1024 * 1024):
    """Construct PE32+ headers independently of the packager's parser."""
    data = bytearray(512)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 128)
    data[128:132] = b"PE\0\0"
    struct.pack_into("<HH", data, 132, machine, 1)
    struct.pack_into("<H", data, 148, 240)
    struct.pack_into("<H", data, 152, 0x20B)
    struct.pack_into("<H", data, 152 + 68, subsystem)
    struct.pack_into("<Q", data, 152 + 72, stack)
    return bytes(data) + commit.encode("ascii") + b"\0" + download_base.encode("ascii")


def pe_with_import(commit, download_base, dll, *, delayed=False):
    data = bytearray(minimal_pe(commit, download_base))
    data.extend(bytes(1536 - len(data)))
    optional = 152
    struct.pack_into("<I", data, optional + 108, 16)
    section = optional + 240
    data[section:section + 8] = b".idata\0\0"
    struct.pack_into("<IIII", data, section + 8, 512, 0x1000, 512, 1024)
    index, size = (13, 64) if delayed else (1, 40)
    struct.pack_into("<II", data, optional + 112 + index * 8, 0x1000, size)
    if delayed:
        struct.pack_into("<8I", data, 1024, 1, 0x1080, 0, 0, 0, 0, 0, 0)
    else:
        struct.pack_into("<5I", data, 1024, 0, 0, 0, 0x1080, 0)
    encoded = dll.encode("ascii") + b"\0"
    data[1152:1152 + len(encoded)] = encoded
    return bytes(data)


class PlayerReleaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not MODULE_PATH.exists():
            return
        spec = importlib.util.spec_from_file_location("build_player_release", MODULE_PATH)
        cls.packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.packager)

    def setUp(self):
        self.assertTrue(MODULE_PATH.is_file(), "player release packager has not been implemented")
        self.temp = tempfile.TemporaryDirectory(prefix="eqoa release test ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "source with spaces"
        self.root.mkdir()
        files = {
            "VERSION": "0.1.0\n", "LICENSE": "existing MIT notice\n",
            "docs/licenses/FrontiersForge-MIT.txt": "existing Forge MIT\n",
            "docs/licenses/Native-Addon-Runtime-NOTICES.txt": "existing Lua and ImGui notices\n",
            "crates/eqoa/resources/map-ui/OFL.txt": "existing Space Mono OFL\n",
            "docs/PLAYER-README.txt": "EQOA Enhanced {version}\nDouble-click eqoa.exe\n",
            "FORK-UPDATE-NOTES.md": "Enhanced release notes\n",
            "mods/README.txt": "EQOA Native - Mods & Addons\n",
            ".gitignore": "target/\nrun/\n",
        }
        for name, content in files.items():
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(content, encoding="utf-8")
        source_lines = [
            "Origin: https://github.com/mmvest/FrontiersForge",
            "Revision: 4026d5cebcede24943ece8dc34833e584f46cd90 (v0.3.2)",
            "",
            "SHA-256 checksums:",
        ]
        for rel in BUNDLED_MOD_FILES:
            mod_bytes = f"content of {rel}\n".encode("utf-8")
            dest = self.root / "bundled-mods" / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(mod_bytes)
            digest = hashlib.sha256(mod_bytes).hexdigest()
            source_lines.append(f"{digest}  {rel}")
        (self.root / "bundled-mods" / "SOURCE.txt").write_text("\n".join(source_lines) + "\n", encoding="utf-8")
        self.git("init", "-q")
        self.git("-c", "user.name=Release Test", "-c", "user.email=test@example.invalid", "add", ".")
        self.git("-c", "user.name=Release Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "fixture")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.executable = self.root / "target" / "eqoa.exe"
        self.executable.parent.mkdir()
        self.base = "https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/"
        self.executable.write_bytes(minimal_pe(self.commit, self.base))
        self.catalog = Path(self.temp.name) / "catalog.json"
        self.catalog.write_bytes(b'{"fixture": "historical catalog"}\n')
        # Only external bytes are substituted. Hash validation and the ZIP are real.
        self.catalog_sha = hashlib.sha256(self.catalog.read_bytes()).hexdigest()
        patch = mock.patch.object(self.packager, "CATALOG_SHA256", self.catalog_sha)
        patch.start()
        self.addCleanup(patch.stop)
        self.output = self.root / "run" / "player release"

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args], text=True, stderr=subprocess.STDOUT)

    def build(self, **kwargs):
        options = dict(root=self.root, executable=self.executable, output=self.output,
                       catalog=self.catalog, commit=None, version=None)
        options.update(kwargs)
        return self.packager.build_release(**options)

    def test_exact_player_archive_and_hash_bound_update_assets(self):
        self.build()
        self.assertEqual({p.name for p in self.output.iterdir()}, {
            "EQOA-Enhanced-0.1.0-Windows-x64.zip", "eqoa-enhanced-windows-x86_64.exe",
            "update.txt", "update-notes.md", "SHA256SUMS"})
        with zipfile.ZipFile(self.output / "EQOA-Enhanced-0.1.0-Windows-x64.zip") as archive:
            self.assertEqual(sorted(archive.namelist()), sorted(EXPECTED_ZIP_MEMBERS))
            manifest = json.loads(archive.read("release-manifest.json"))
            self.assertEqual(manifest["version"], "0.1.0")
            self.assertEqual(manifest["sourceCommit"], self.commit)
            self.assertFalse(manifest["sourceModified"])
            self.assertEqual(manifest["updaterChannel"], "enhanced")
            self.assertEqual(manifest["catalog"]["revision"], "4026d5cebcede24943ece8dc34833e584f46cd90")
            self.assertEqual(manifest["catalog"]["sha256"], self.catalog_sha)
            for name, digest in manifest["files"].items():
                self.assertEqual(hashlib.sha256(archive.read(name)).hexdigest(), digest)
            self.assertEqual(set(manifest["files"]), set(archive.namelist()) - {"release-manifest.json"})
            self.assertEqual(archive.read("eqoa.exe"), self.executable.read_bytes())
        exe = self.executable.read_bytes()
        self.assertEqual((self.output / "update.txt").read_text(),
            f"version 0.1.0\ncommit {self.commit}\nbuild windows-x86_64 eqoa-enhanced-windows-x86_64.exe {hashlib.sha256(exe).hexdigest()} {len(exe)}\n")
        self.assertEqual((self.output / "update-notes.md").read_bytes(), (self.root / "FORK-UPDATE-NOTES.md").read_bytes())
        sums = (self.output / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(sums), 4)
        for row in sums:
            digest, name = row.split("  ")
            self.assertEqual(hashlib.sha256((self.output / name).read_bytes()).hexdigest(), digest)

    def test_player_readme_uses_the_packaged_version(self):
        (self.root / "VERSION").write_text("0.1.1\n", encoding="utf-8")
        self.git("add", "VERSION")
        self.git("-c", "user.name=Release Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "next release")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.executable.write_bytes(minimal_pe(self.commit, self.base))
        self.build()
        with zipfile.ZipFile(self.output / "EQOA-Enhanced-0.1.1-Windows-x64.zip") as archive:
            self.assertEqual(archive.read("README.txt").decode("utf-8").splitlines(), ["EQOA Enhanced 0.1.1", "Double-click eqoa.exe"])

    def test_independent_verifier_accepts_the_complete_release(self):
        self.assertTrue(callable(getattr(self.packager, "verify_release", None)),
                        "independent release verification has not been implemented")
        self.build()
        self.assertEqual(self.packager.verify_release(self.output)["sourceCommit"], self.commit)

    def test_independent_verifier_rejects_tampering_even_with_recomputed_outer_hashes(self):
        self.assertTrue(callable(getattr(self.packager, "verify_release", None)),
                        "independent release verification has not been implemented")
        self.build()
        for mutation in ("raw-exe", "update", "extra-asset", "extra-zip-member", "manifest-version", "catalog"):
            with self.subTest(mutation=mutation):
                changed = self.output.parent / mutation
                shutil.copytree(self.output, changed)
                if mutation == "raw-exe":
                    (changed / "eqoa-enhanced-windows-x86_64.exe").write_bytes(b"tampered")
                elif mutation == "update":
                    update = changed / "update.txt"
                    update.write_text(update.read_text().replace("version 0.1.0", "version 0.2.0"))
                elif mutation == "extra-asset":
                    (changed / "accounts.txt").write_text("PRIVATE")
                else:
                    archive_path = changed / "EQOA-Enhanced-0.1.0-Windows-x64.zip"
                    with zipfile.ZipFile(archive_path) as archive:
                        payload = {name: archive.read(name) for name in archive.namelist()}
                    if mutation == "extra-zip-member":
                        payload["profile/accounts.txt"] = b"PRIVATE"
                    elif mutation == "manifest-version":
                        manifest = json.loads(payload["release-manifest.json"])
                        manifest["version"] = "0.2.0"
                        payload["release-manifest.json"] = json.dumps(manifest).encode()
                    else:
                        payload["map-entities.json"] = b"tampered catalog"
                        manifest = json.loads(payload["release-manifest.json"])
                        manifest["files"]["map-entities.json"] = hashlib.sha256(payload["map-entities.json"]).hexdigest()
                        payload["release-manifest.json"] = json.dumps(manifest).encode()
                    with zipfile.ZipFile(archive_path, "w") as archive:
                        for name, data in payload.items():
                            archive.writestr(name, data)
                # Deep checks must reject tampering even after an attacker updates outer hashes.
                sums = changed / "SHA256SUMS"
                names = [row.split("  ")[1] for row in sums.read_text().splitlines()]
                sums.write_text("".join(f"{hashlib.sha256((changed / name).read_bytes()).hexdigest()}  {name}\n" for name in names))
                with self.assertRaises(ValueError):
                    self.packager.verify_release(changed)

    def test_private_files_never_enter_archive_from_source(self):
        for name in ("run/accounts.txt", "run/profile/game.opt", "run/owned.iso"):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(b"PRIVATE")
        self.build()
        with zipfile.ZipFile(self.output / "EQOA-Enhanced-0.1.0-Windows-x64.zip") as archive:
            self.assertNotIn(b"PRIVATE", b"".join(archive.read(name) for name in archive.namelist()))

    def test_poisoned_output_is_rejected_without_deleting_private_files(self):
        self.output.mkdir(parents=True)
        private = self.output / "account-profile.iso"
        private.write_bytes(b"PRIVATE")
        with self.assertRaisesRegex(ValueError, "empty"):
            self.build()
        self.assertEqual(private.read_bytes(), b"PRIVATE")
        self.assertEqual(list(self.output.iterdir()), [private])

    def test_wrong_catalog_is_rejected_before_output(self):
        self.catalog.write_bytes(b"wrong catalog")
        with self.assertRaisesRegex(ValueError, "catalog|Catalog"):
            self.build()
        self.assertFalse(self.output.exists())

    def test_invalid_versions_and_mismatched_override_are_rejected(self):
        for version in ("01.1.0", "0.01.0", "0.1.00", "v0.1.0", "0.1", "0.1.0-rc1", "0.1.0+test", "0.1.0\nextra"):
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, "version|VERSION"):
                self.build(version=version)
        with self.assertRaisesRegex(ValueError, "match|VERSION"):
            self.build(version="0.2.0")

    def test_wrong_commit_override_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "commit|HEAD"):
            self.build(commit="1" * 40)

    def test_dirty_source_and_untracked_files_are_rejected(self):
        (self.root / "VERSION").write_text("0.1.0\n\n")
        with self.assertRaisesRegex(ValueError, "clean|dirty|modified"):
            self.build()
        self.git("checkout", "--", "VERSION")
        (self.root / "untracked-account.txt").write_text("PRIVATE")
        with self.assertRaisesRegex(ValueError, "clean|dirty|modified"):
            self.build()

    def test_executable_must_match_commit_channel_and_windows_contract(self):
        invalid = [b"not a PE", minimal_pe("0" * 40, self.base),
                   minimal_pe(self.commit, "https://github.com/DabDavis/eqoa/releases/latest/download/"),
                   minimal_pe(self.commit, self.base, machine=0x14C),
                   minimal_pe(self.commit, self.base, subsystem=3),
                   minimal_pe(self.commit, self.base, stack=1024 * 1024)]
        for executable in invalid:
            with self.subTest(executable=executable[-80:]), self.assertRaises(ValueError):
                self.executable.write_bytes(executable)
                self.build()
        self.assertFalse(self.output.exists())

    def test_executable_rejects_regular_and_delayed_visual_cpp_runtime_imports(self):
        for dll in ("VCRUNTIME140.dll", "vcruntime140_1.dll", "MSVCP140.dll", "msvcr120.dll", "concrt140.dll", "vcomp140.dll", "ucrtbased.dll"):
            for delayed in (False, True):
                with self.subTest(dll=dll, delayed=delayed):
                    self.executable.write_bytes(pe_with_import(self.commit, self.base, dll, delayed=delayed))
                    with self.assertRaisesRegex(ValueError, "runtime|Runtime"):
                        self.build()
        self.assertFalse(self.output.exists())

    def test_executable_accepts_system_imports_and_no_imports(self):
        self.packager.validate_executable(minimal_pe(self.commit, self.base), self.commit)
        for delayed in (False, True):
            with self.subTest(delayed=delayed):
                self.packager.validate_executable(pe_with_import(self.commit, self.base, "KERNEL32.dll", delayed=delayed), self.commit)

    def test_input_inside_output_and_output_inside_source_files_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "output|Output"):
            self.build(output=self.root)
        with self.assertRaisesRegex(ValueError, "output|Output"):
            self.build(output=self.executable.parent)


    def test_tampered_bundled_mod_file_fails(self):
        tampered = self.root / "bundled-mods" / BUNDLED_MOD_FILES[0]
        tampered.write_bytes(b"tampered content\n")
        self.git("add", ".")
        self.git("-c", "user.name=Release Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "tampered mod")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.executable.write_bytes(minimal_pe(self.commit, self.base))
        with self.assertRaisesRegex(ValueError, "checksum|tampered|mismatch"):
            self.build()

    def test_missing_bundled_mod_file_fails(self):
        missing = self.root / "bundled-mods" / BUNDLED_MOD_FILES[0]
        missing.unlink()
        self.git("add", ".")
        self.git("-c", "user.name=Release Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "missing mod")
        self.commit = self.git("rev-parse", "HEAD").strip()
        self.executable.write_bytes(minimal_pe(self.commit, self.base))
        with self.assertRaisesRegex(ValueError, "missing|Missing"):
            self.build()

    def test_extra_file_in_zip_fails_verification(self):
        self.build()
        archive_path = self.output / "EQOA-Enhanced-0.1.0-Windows-x64.zip"
        with zipfile.ZipFile(archive_path) as archive:
            payload = {name: archive.read(name) for name in archive.namelist()}
        payload["mods/extra_unapproved.lua"] = b"-- unapproved"
        manifest = json.loads(payload["release-manifest.json"])
        manifest["files"]["mods/extra_unapproved.lua"] = hashlib.sha256(b"-- unapproved").hexdigest()
        payload["release-manifest.json"] = json.dumps(manifest).encode()
        with zipfile.ZipFile(archive_path, "w") as archive:
            for name, data in payload.items():
                archive.writestr(name, data)
        sums = self.output / "SHA256SUMS"
        names = [row.split("  ")[1] for row in sums.read_text().splitlines()]
        sums.write_text("".join(f"{hashlib.sha256((self.output / name).read_bytes()).hexdigest()}  {name}\n" for name in names))
        with self.assertRaisesRegex(ValueError, "allowlist|member"):
            self.packager.verify_release(self.output)


if __name__ == "__main__":
    unittest.main()
