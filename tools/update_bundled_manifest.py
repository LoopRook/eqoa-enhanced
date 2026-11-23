#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Update bundled mod file lists and SHA-256 checksums in SOURCE.txt and release tooling."""
import argparse
import difflib
import hashlib
from pathlib import Path
import re
import subprocess
import sys

RE_BUILD_MOD_FILES = re.compile(r"BUNDLED_MOD_FILES\s*=\s*\([^\)]*\)", re.DOTALL)
RE_TEST_MOD_FILES = re.compile(r"BUNDLED_MOD_FILES\s*=\s*\[[^\]]*\]", re.DOTALL)
RE_CHECKSUM_LINE = re.compile(r"^[0-9a-f]{64}\s+\S+")

ALLOWED_EXTENSIONS = {".json", ".lua", ".md", ".png", ".ttf", ".txt"}
# Files the client ships and the mutation tests read as text: a CR byte means the file was saved CRLF.
TEXT_EXTENSIONS = {".json", ".lua", ".md", ".txt"}
# Only the fork's own mods (eqoa_*) must be LF. Vendored FrontiersForge mods and font notices stay byte for byte as
# shipped upstream (CRLF), so they are not checked; neither is any `resources` folder.


def find_files_with_cr(bundled_dir):
    """Every text file of a fork-owned mod (`eqoa_*`, outside `resources`) that contains a CR byte.

    Files under bundled-mods are `-text` in .gitattributes (stored byte for byte), so a CRLF save stays CRLF and
    breaks the tests that match LF source. The names are returned relative to `bundled_dir`, sorted.
    """
    found = []
    for path in sorted(Path(bundled_dir).rglob("*")):
        if not (path.is_file() and path.suffix.lower() in TEXT_EXTENSIONS):
            continue
        rel = path.relative_to(bundled_dir)
        if not rel.parts[0].startswith("eqoa_") or "resources" in rel.parts[1:-1]:
            continue
        if b"\r" in path.read_bytes():
            found.append(rel.as_posix())
    return found


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def detect_eol(raw_bytes):
    return "\r\n" if b"\r\n" in raw_bytes else "\n"


def get_git_file_candidates(root, bundled_dir):
    candidates = set()
    try:
        res = subprocess.run(
            ["git", "-c", "safe.directory=*", "-C", str(root), "ls-files", "bundled-mods"],
            capture_output=True,
            text=True,
            check=True,
        )
        for line in res.stdout.splitlines():
            line = line.strip()
            if line:
                candidates.add(line.replace("\\", "/"))

        res_others = subprocess.run(
            ["git", "-c", "safe.directory=*", "-C", str(root), "ls-files", "--others", "--exclude-standard", "bundled-mods"],
            capture_output=True,
            text=True,
            check=True,
        )
        for line in res_others.stdout.splitlines():
            line = line.strip()
            if line:
                candidates.add(line.replace("\\", "/"))
    except Exception:
        # Fallback if git is not available (e.g. non-git directory fixture)
        for p in bundled_dir.rglob("*"):
            if p.is_file():
                rel_from_root = p.relative_to(root).as_posix()
                candidates.add(rel_from_root)
    return candidates


def scan_bundled_mod_files(root, bundled_dir):
    bundled_dir = Path(bundled_dir).resolve()
    root = Path(root).resolve()

    # Allowed top-level directories under bundled-mods
    existing_mod_folders = {p.name for p in bundled_dir.iterdir() if p.is_dir()}
    git_candidates = get_git_file_candidates(root, bundled_dir)

    mod_files = []
    for candidate in sorted(git_candidates):
        try:
            full_path = (root / candidate).resolve()
            rel_to_bundled = full_path.relative_to(bundled_dir).as_posix()
        except ValueError:
            continue

        if not full_path.is_file():
            continue

        parts = rel_to_bundled.split("/")
        # Must be inside an existing mod/resource folder (not at root of bundled-mods)
        if len(parts) < 2 or parts[0] not in existing_mod_folders:
            continue

        # Must not be dotfiles or editor temp/backup files
        if any(part.startswith(".") for part in parts):
            continue
        filename = parts[-1]
        if filename.startswith("#") or filename.endswith("~"):
            continue

        # Must have an allowed extension from the runtime allowlist
        suffix = full_path.suffix.lower()
        if suffix not in ALLOWED_EXTENSIONS:
            continue

        mod_files.append((rel_to_bundled, full_path))

    return mod_files


def parse_existing_bundled_files(text, is_tuple=True):
    pattern = RE_BUILD_MOD_FILES if is_tuple else RE_TEST_MOD_FILES
    match = pattern.search(text)
    if not match:
        return []
    block = match.group(0)
    return re.findall(r'"([^"]+)"', block)


def order_mod_files(existing_files, disk_files_set):
    ordered = [f for f in existing_files if f in disk_files_set]
    existing_set = set(ordered)

    new_files = sorted(disk_files_set - existing_set)
    for new_file in new_files:
        pkg = new_file.split("/")[0]
        last_pkg_idx = -1
        for i, f in enumerate(ordered):
            if f.split("/")[0] == pkg:
                last_pkg_idx = i

        if last_pkg_idx != -1:
            ordered.insert(last_pkg_idx + 1, new_file)
        else:
            res_idx = -1
            for i, f in enumerate(ordered):
                if f.startswith("resources/"):
                    res_idx = i
                    break
            if res_idx != -1:
                ordered.insert(res_idx, new_file)
            else:
                ordered.append(new_file)

    return ordered


def format_source_txt(original_text, disk_files_with_hashes, eol="\n"):
    sorted_entries = sorted(disk_files_with_hashes, key=lambda x: x[0])
    if "SHA-256 checksums:" not in original_text:
        header = original_text.rstrip("\r\n") + eol + eol + "SHA-256 checksums:" + eol
        suffix = ""
    else:
        header_part, rest = original_text.split("SHA-256 checksums:", 1)
        header = header_part + "SHA-256 checksums:" + eol
        rest_lines = rest.splitlines(keepends=True)
        idx = 0
        while idx < len(rest_lines) and not rest_lines[idx].strip():
            idx += 1
        while idx < len(rest_lines) and RE_CHECKSUM_LINE.match(rest_lines[idx].strip()):
            idx += 1
        suffix = "".join(rest_lines[idx:])

    checksum_lines = [f"{digest}  {rel}" for rel, digest in sorted_entries]
    checksum_block = eol.join(checksum_lines) + eol
    return header + checksum_block + suffix


def format_python_file(original_text, ordered_files, is_tuple=True, eol="\n"):
    pattern = RE_BUILD_MOD_FILES if is_tuple else RE_TEST_MOD_FILES
    open_char = "(" if is_tuple else "["
    close_char = ")" if is_tuple else "]"

    lines = [f"BUNDLED_MOD_FILES = {open_char}"]
    for f in ordered_files:
        lines.append(f'    "{f}",')
    lines.append(close_char)
    new_block = eol.join(lines)

    if not pattern.search(original_text):
        raise ValueError("Could not find BUNDLED_MOD_FILES block in file")

    return pattern.sub(new_block, original_text)


def update_bundled_manifests(root=None, check=False):
    if root is None:
        root = Path.cwd()
    root = Path(root).resolve()

    bundled_dir = root / "bundled-mods"
    source_txt_path = bundled_dir / "SOURCE.txt"
    build_py_path = root / "tools" / "build_player_release.py"
    test_py_path = root / "tools" / "test_player_release.py"

    if not bundled_dir.is_dir():
        raise FileNotFoundError(f"bundled-mods directory not found at {bundled_dir}")
    if not source_txt_path.is_file():
        raise FileNotFoundError(f"SOURCE.txt not found at {source_txt_path}")
    if not build_py_path.is_file():
        raise FileNotFoundError(f"build_player_release.py not found at {build_py_path}")
    if not test_py_path.is_file():
        raise FileNotFoundError(f"test_player_release.py not found at {test_py_path}")

    mod_files = scan_bundled_mod_files(root, bundled_dir)
    disk_files_with_hashes = [(rel, sha256_file(p)) for rel, p in mod_files]
    disk_files_set = {rel for rel, _ in disk_files_with_hashes}

    old_source_bytes = source_txt_path.read_bytes()
    old_build_bytes = build_py_path.read_bytes()
    old_test_bytes = test_py_path.read_bytes()

    source_eol = detect_eol(old_source_bytes)
    build_eol = detect_eol(old_build_bytes)
    test_eol = detect_eol(old_test_bytes)

    old_source_txt = old_source_bytes.decode("utf-8")
    old_build_py = old_build_bytes.decode("utf-8")
    old_test_py = old_test_bytes.decode("utf-8")

    new_source_txt = format_source_txt(old_source_txt, disk_files_with_hashes, eol=source_eol)

    existing_build_files = parse_existing_bundled_files(old_build_py, is_tuple=True)
    ordered_files = order_mod_files(existing_build_files, disk_files_set)

    new_build_py = format_python_file(old_build_py, ordered_files, is_tuple=True, eol=build_eol)
    new_test_py = format_python_file(old_test_py, ordered_files, is_tuple=False, eol=test_eol)

    new_source_bytes = new_source_txt.encode("utf-8")
    new_build_bytes = new_build_py.encode("utf-8")
    new_test_bytes = new_test_py.encode("utf-8")

    diffs = []
    for path, old_b, new_b in [
        (source_txt_path, old_source_bytes, new_source_bytes),
        (build_py_path, old_build_bytes, new_build_bytes),
        (test_py_path, old_test_bytes, new_test_bytes),
    ]:
        if old_b != new_b:
            old_str = old_b.decode("utf-8", errors="replace")
            new_str = new_b.decode("utf-8", errors="replace")
            diff = list(difflib.unified_diff(
                old_str.splitlines(keepends=True),
                new_str.splitlines(keepends=True),
                fromfile=str(path.relative_to(root)),
                tofile=str(path.relative_to(root)),
            ))
            diffs.append((path, "".join(diff), new_b))

    if check:
        cr_files = find_files_with_cr(bundled_dir)
        if cr_files:
            print(f"Bundled mod text files contain CR bytes ({len(cr_files)} file(s)); keep them LF:")
            for rel in cr_files:
                print(f"  bundled-mods/{rel}")
            return 1
        if diffs:
            print(f"Bundled mod manifests are out of date ({len(diffs)} file(s) differ):")
            for _, d, _ in diffs:
                sys.stdout.write(d)
            return 1
        print("Bundled mod manifests are up to date.")
        return 0

    if diffs:
        for path, _, new_b in diffs:
            path.write_bytes(new_b)
        print(f"Updated {len(diffs)} bundled manifest file(s):")
        for path, _, _ in diffs:
            print(f"  {path.relative_to(root)}")
    else:
        print("Bundled mod manifests are already up to date; no changes written.")

    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root (default: current directory)")
    parser.add_argument("--check", action="store_true", help="verify manifests without modifying files; exit 1 if out of date")
    args = parser.parse_args()

    return update_bundled_manifests(root=args.root, check=args.check)


if __name__ == "__main__":
    sys.exit(main())
