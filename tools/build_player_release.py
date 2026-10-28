#!/usr/bin/env python3
"""Package an already built Enhanced Windows client using an explicit allowlist.

Run from a clean checkout after building with EQOA_UPDATE_CHANNEL=enhanced and
EQOA_BUILD_COMMIT set to its full Git HEAD. No build or source copying occurs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile
import urllib.request
import zipfile

CATALOG_REVISION = "4026d5cebcede24943ece8dc34833e584f46cd90"
CATALOG_SHA256 = "37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca"
CATALOG_URL = (
    "https://raw.githubusercontent.com/mmvest/FrontiersForge/"
    f"{CATALOG_REVISION}/scripts/resources/entity_database.json"
)
DOWNLOAD_BASE = "https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/"
UPDATER_ASSET = "eqoa-enhanced-windows-x86_64.exe"
VERSION_PATTERN = re.compile(r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", re.ASCII)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git(root, *args):
    result = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)
    if result.returncode:
        raise ValueError(f"Git source verification failed: {result.stderr.strip()}")
    return result.stdout.strip()


def source_commit(root):
    if Path(git(root, "rev-parse", "--show-toplevel")).resolve() != root:
        raise ValueError("--root must be the Git checkout root")
    commit = git(root, "rev-parse", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Git HEAD must be a full SHA-1 commit")
    if git(root, "status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("Release source must be clean: modified or untracked files found")
    return commit


def imported_dlls(data, pe_offset, optional_size):
    """Read regular and delayed PE32+ imports, resolving RVAs through sections."""
    optional = pe_offset + 24
    if optional_size < 112:
        raise ValueError("Executable has truncated PE32+ data directories")
    directory_count = struct.unpack_from("<I", data, optional + 108)[0]
    if directory_count > (optional_size - 112) // 8:
        raise ValueError("Executable has truncated PE import directories")
    section_count = struct.unpack_from("<H", data, pe_offset + 6)[0]
    section_table = optional + optional_size
    if section_table + section_count * 40 > len(data):
        raise ValueError("Executable has a truncated PE section table")
    headers_size = struct.unpack_from("<I", data, optional + 60)[0]
    image_base = struct.unpack_from("<Q", data, optional + 24)[0]

    def rva_offset(rva, size):
        if rva < headers_size and rva + size <= min(headers_size, len(data)):
            return rva
        for index in range(section_count):
            section = section_table + index * 40
            virtual_size, address, raw_size, raw_offset = struct.unpack_from("<IIII", data, section + 8)
            if address <= rva < address + max(virtual_size, raw_size):
                delta = rva - address
                offset = raw_offset + delta
                if delta + size <= raw_size and offset + size <= len(data):
                    return offset
                break
        raise ValueError("Executable import RVA has no complete file-backed section")

    def dll_name(rva):
        if not rva:
            raise ValueError("Executable import is missing its DLL name")
        name = bytearray()
        for index in range(1024):
            value = data[rva_offset(rva + index, 1)]
            if value == 0:
                try:
                    result = name.decode("ascii")
                except UnicodeDecodeError as error:
                    raise ValueError("Executable import DLL name is not ASCII") from error
                if not result or "/" in result or "\\" in result:
                    raise ValueError("Executable import has an invalid DLL name")
                return result
            name.append(value)
        raise ValueError("Executable import DLL name is not terminated")

    imports = []
    for directory, stride in ((1, 20), (13, 32)):
        if directory >= directory_count:
            continue
        rva, size = struct.unpack_from("<II", data, optional + 112 + directory * 8)
        if rva == 0 and size == 0:
            continue
        if not rva or size < stride:
            raise ValueError("Executable has an invalid PE import directory")
        for position in range(0, size - stride + 1, stride):
            descriptor = rva_offset(rva + position, stride)
            fields = struct.unpack_from("<" + "I" * (stride // 4), data, descriptor)
            if not any(fields):
                break
            if directory == 1:
                name_rva = fields[3]
            else:
                name_rva = fields[1] if fields[0] & 1 else fields[1] - image_base
            imports.append(dll_name(name_rva))
        else:
            raise ValueError("Executable import directory is not terminated")
    return imports


def validate_executable(data, commit):
    """Inspect PE32+ headers without depending on platform-specific PE tooling."""
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("Executable is not a Windows PE file")
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if offset + 24 > len(data) or data[offset:offset + 4] != b"PE\0\0":
        raise ValueError("Executable has an invalid PE signature")
    machine = struct.unpack_from("<H", data, offset + 4)[0]
    optional_size = struct.unpack_from("<H", data, offset + 20)[0]
    optional = offset + 24
    if machine != 0x8664 or optional_size < 80 or optional + optional_size > len(data):
        raise ValueError("Executable must have complete Windows x64 PE headers")
    magic = struct.unpack_from("<H", data, optional)[0]
    subsystem = struct.unpack_from("<H", data, optional + 68)[0]
    stack_reserve = struct.unpack_from("<Q", data, optional + 72)[0]
    if magic != 0x20B or subsystem != 2:
        raise ValueError("Executable must be a PE32+ Windows GUI application")
    if stack_reserve < 8 * 1024 * 1024:
        raise ValueError("Executable must reserve at least 8 MiB of stack")
    runtime_prefixes = ("vcruntime", "msvcp", "msvcr", "msvcm", "concrt", "vcomp")
    for dll in imported_dlls(data, offset, optional_size):
        name = dll.lower()
        # MSVCRT is an OS component; versioned Microsoft C/C++ runtimes require
        # a redistributable installation and cannot accompany an EXE-only build.
        if name == "ucrtbased.dll" or (name != "msvcrt.dll" and name.startswith(runtime_prefixes)):
            raise ValueError(f"Executable imports an unbundled Visual C++ runtime: {dll}; build with +crt-static")
    if commit.encode("ascii") not in data:
        raise ValueError("Executable does not contain the full source commit")
    if DOWNLOAD_BASE.encode("ascii") not in data:
        raise ValueError("Executable does not contain the Enhanced updater download base")


def build_release(*, root, executable, output, catalog=None, commit=None, version=None):
    root = Path(root).resolve()
    executable = Path(executable).resolve()
    output = Path(output).resolve()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        raise ValueError("Output directory must be empty; refusing to overwrite existing files")
    source = source_commit(root)
    if commit is not None and commit != source:
        raise ValueError("Requested commit must match the full Git HEAD")
    recorded_version = (root / "VERSION").read_text(encoding="utf-8").removesuffix("\n")
    if not VERSION_PATTERN.fullmatch(recorded_version):
        raise ValueError("VERSION must contain a canonical MAJOR.MINOR.PATCH version")
    if version is not None and (not VERSION_PATTERN.fullmatch(version) or version != recorded_version):
        raise ValueError("Requested version must match canonical VERSION")
    version = recorded_version
    inputs = {
        "eqoa.exe": executable,
        "LICENSE": root / "LICENSE",
        "FrontiersForge-MIT.txt": root / "docs/licenses/FrontiersForge-MIT.txt",
        "SpaceMono-OFL.txt": root / "crates/eqoa/resources/map-ui/OFL.txt",
        "README.txt": root / "docs/PLAYER-README.txt",
    }
    notes_path = root / "FORK-UPDATE-NOTES.md"
    source_paths = [*inputs.values(), notes_path, root / "VERSION"]
    if catalog is not None:
        catalog = Path(catalog).resolve()
        source_paths.append(catalog)
    if any(path.resolve().is_relative_to(output) for path in source_paths):
        raise ValueError("Output directory must not contain a release input")
    exe = executable.read_bytes()
    validate_executable(exe, source)
    if catalog is None:
        with urllib.request.urlopen(CATALOG_URL, timeout=60) as response:
            catalog_bytes = response.read()
    else:
        catalog_bytes = catalog.read_bytes()
    if sha256(catalog_bytes) != CATALOG_SHA256:
        raise ValueError("Catalog does not match the pinned FrontiersForge SHA256")
    payload = {"eqoa.exe": exe, "map-entities.json": catalog_bytes}
    payload.update({name: path.read_bytes() for name, path in inputs.items() if name != "eqoa.exe"})
    payload["README.txt"] = payload["README.txt"].replace(b"{version}", version.encode("ascii"))
    manifest = {
        "version": version,
        "sourceCommit": source,
        "sourceModified": False,
        "platform": "windows-x86_64",
        "updaterChannel": "enhanced",
        "downloadBase": DOWNLOAD_BASE,
        "catalog": {"revision": CATALOG_REVISION, "url": CATALOG_URL, "sha256": CATALOG_SHA256},
        # The manifest cannot include its own hash; the ZIP is covered by SHA256SUMS.
        "files": {name: sha256(data) for name, data in payload.items()},
    }
    payload["release-manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode("utf-8")
    notes = notes_path.read_bytes()
    if source_commit(root) != source:
        raise ValueError("Git HEAD changed while packaging")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".eqoa-release-", dir=output.parent) as temporary:
        stage = Path(temporary)
        zip_name = f"EQOA-Enhanced-{version}-Windows-x64.zip"
        with zipfile.ZipFile(stage / zip_name, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name, data in payload.items():
                entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.create_system = 3
                entry.external_attr = 0o100644 << 16
                archive.writestr(entry, data)
        (stage / UPDATER_ASSET).write_bytes(exe)
        (stage / "update.txt").write_bytes((
            f"version {version}\ncommit {source}\n"
            f"build windows-x86_64 {UPDATER_ASSET} {sha256(exe)} {len(exe)}\n"
        ).encode("ascii"))
        (stage / "update-notes.md").write_bytes(notes)
        assets = [zip_name, UPDATER_ASSET, "update.txt", "update-notes.md"]
        (stage / "SHA256SUMS").write_bytes("".join(
            f"{sha256((stage / name).read_bytes())}  {name}\n" for name in assets
        ).encode("ascii"))
        verify_release(stage)
        output.mkdir(exist_ok=True)
        if any(output.iterdir()):
            raise ValueError("Output directory must remain empty while packaging")
        for name in [*assets, "SHA256SUMS"]:
            (stage / name).replace(output / name)
    return output / zip_name


def verify_release(output):
    """Independently verify finished assets, returning the verified manifest dict.

    No Git checkout or build inputs are required. The caller can verify the exact
    directory downloaded from CI before allowing publication.
    """
    output = Path(output)
    if not output.is_dir() or any(not path.is_file() or path.is_symlink() for path in output.iterdir()):
        raise ValueError("Release output must contain only regular asset files")
    update = (output / "update.txt").read_bytes()
    try:
        rows = update.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        raise ValueError("Update manifest must be ASCII") from error
    if len(rows) != 3:
        raise ValueError("Update manifest must contain version, commit and build rows")
    version_row = rows[0].split(" ")
    commit_row = rows[1].split(" ")
    build_row = rows[2].split(" ")
    if len(version_row) != 2 or version_row[0] != "version" or not VERSION_PATTERN.fullmatch(version_row[1]):
        raise ValueError("Update manifest has an invalid semantic version")
    if len(commit_row) != 2 or commit_row[0] != "commit" or not re.fullmatch(r"[0-9a-f]{40}", commit_row[1]):
        raise ValueError("Update manifest has an invalid source commit")
    if (len(build_row) != 5 or build_row[:3] != ["build", "windows-x86_64", UPDATER_ASSET]
            or not re.fullmatch(r"[0-9a-f]{64}", build_row[3])
            or not re.fullmatch(r"[1-9][0-9]*", build_row[4])):
        raise ValueError("Update manifest has an invalid Windows build row")
    version, commit = version_row[1], commit_row[1]
    zip_name = f"EQOA-Enhanced-{version}-Windows-x64.zip"
    asset_names = {zip_name, UPDATER_ASSET, "update.txt", "update-notes.md"}
    if {path.name for path in output.iterdir()} != asset_names | {"SHA256SUMS"}:
        raise ValueError("Release output asset allowlist mismatch")
    checksums = {}
    for row in (output / "SHA256SUMS").read_text(encoding="ascii").splitlines():
        parts = row.split("  ")
        if (len(parts) != 2 or not re.fullmatch(r"[0-9a-f]{64}", parts[0])
                or parts[1] not in asset_names or parts[1] in checksums):
            raise ValueError("Invalid release checksum row")
        checksums[parts[1]] = parts[0]
    if set(checksums) != asset_names:
        raise ValueError("Release checksum asset allowlist mismatch")
    for name, digest in checksums.items():
        if sha256((output / name).read_bytes()) != digest:
            raise ValueError(f"Release checksum mismatch: {name}")
    exe = (output / UPDATER_ASSET).read_bytes()
    validate_executable(exe, commit)
    if build_row[3] != sha256(exe) or int(build_row[4]) != len(exe):
        raise ValueError("Update manifest executable hash or size mismatch")
    expected_update = (f"version {version}\ncommit {commit}\n"
        f"build windows-x86_64 {UPDATER_ASSET} {sha256(exe)} {len(exe)}\n").encode("ascii")
    if update != expected_update:
        raise ValueError("Update manifest is not in canonical format")
    members = {"eqoa.exe", "map-entities.json", "LICENSE", "FrontiersForge-MIT.txt",
               "SpaceMono-OFL.txt", "README.txt", "release-manifest.json"}
    with zipfile.ZipFile(output / zip_name) as archive:
        if len(archive.infolist()) != len(members) or set(archive.namelist()) != members:
            raise ValueError("Player ZIP member allowlist mismatch")
        if any(info.is_dir() or (info.external_attr >> 16) & 0o170000 == 0o120000 for info in archive.infolist()):
            raise ValueError("Player ZIP must contain regular files")
        payload = {name: archive.read(name) for name in members}
    try:
        manifest = json.loads(payload["release-manifest.json"])
    except (ValueError, UnicodeDecodeError) as error:
        raise ValueError("Invalid release manifest JSON") from error
    if not isinstance(manifest, dict):
        raise ValueError("Release manifest must be a JSON object")
    expected_fields = {"version": version, "sourceCommit": commit, "sourceModified": False,
                       "platform": "windows-x86_64", "updaterChannel": "enhanced", "downloadBase": DOWNLOAD_BASE}
    if any(manifest.get(key) != value for key, value in expected_fields.items()):
        raise ValueError("Release manifest provenance or version mismatch")
    if manifest.get("catalog") != {"revision": CATALOG_REVISION, "url": CATALOG_URL, "sha256": CATALOG_SHA256}:
        raise ValueError("Release manifest catalog provenance mismatch")
    file_hashes = {name: sha256(data) for name, data in payload.items() if name != "release-manifest.json"}
    if manifest.get("files") != file_hashes:
        raise ValueError("Release manifest file hashes mismatch")
    if payload["eqoa.exe"] != exe:
        raise ValueError("Player ZIP executable does not match updater executable")
    if sha256(payload["map-entities.json"]) != CATALOG_SHA256:
        raise ValueError("Player ZIP catalog does not match pinned SHA256")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new or empty release asset directory")
    parser.add_argument("--catalog", type=Path, help="local catalog matching the pinned SHA256; otherwise downloaded")
    parser.add_argument("--commit", help="optional full commit assertion; must match clean Git HEAD")
    parser.add_argument("--version", help="optional version assertion; must match VERSION")
    args = parser.parse_args()
    try:
        archive = build_release(**vars(args))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")
    print(archive)


if __name__ == "__main__":
    main()
