#!/usr/bin/env python3
# Code in this file published in EQOA Enhanced 0.4.5 or earlier is MIT (see NOTICE). LoopRook's additions first published in 0.4.6 or later are under the GNU Affero General Public License v3 or later (see LICENSE); upstream and third-party code stays under its own license.
"""Update bundled mod file lists and SHA-256 checksums in SOURCE.txt and release tooling."""
import argparse
import difflib
import hashlib
from pathlib import Path
import re
import sys

RE_BUILD_MOD_FILES = re.compile(r"BUNDLED_MOD_FILES\s*=\s*\([^\)]*\)", re.DOTALL)
RE_TEST_MOD_FILES = re.compile(r"BUNDLED_MOD_FILES\s*=\s*\[[^\]]*\]", re.DOTALL)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def scan_bundled_mod_files(bundled_dir):
    bundled_dir = Path(bundled_dir)
    mod_files = []
    for p in bundled_dir.rglob("*"):
        if not p.is_file():
            continue
        rel = p.relative_to(bundled_dir).as_posix()
        if rel == "SOURCE.txt" or any(part.startswith(".") for part in rel.split("/")):
            continue
        mod_files.append((rel, p))
    return mod_files


def parse_existing_bundled_files(text, is_tuple=True):
    pattern = RE_BUILD_MOD_FILES if is_tuple else RE_TEST_MOD_FILES
    match = pattern.search(text)
    if not match:
        return []
    block = match.group(0)
    return re.findall(r'"([^"]+)"', block)


def order_mod_files(existing_files, disk_files_set):
    # Retain existing files that still exist on disk
    ordered = [f for f in existing_files if f in disk_files_set]
    existing_set = set(ordered)

    new_files = sorted(disk_files_set - existing_set)
    for new_file in new_files:
        pkg = new_file.split("/")[0]
        # Find the last occurrence of a file from the same package
        last_pkg_idx = -1
        for i, f in enumerate(ordered):
            if f.split("/")[0] == pkg:
                last_pkg_idx = i

        if last_pkg_idx != -1:
            ordered.insert(last_pkg_idx + 1, new_file)
        else:
            # If package not present, insert before 'resources' if possible, else append
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


def format_source_txt(original_text, disk_files_with_hashes):
    # disk_files_with_hashes: list of (rel_path, hash) sorted by posix path
    sorted_entries = sorted(disk_files_with_hashes, key=lambda x: x[0])
    if "SHA-256 checksums:\n" in original_text:
        header, _ = original_text.split("SHA-256 checksums:\n", 1)
    else:
        header = original_text.rstrip() + "\n\n"
    checksum_lines = [f"{digest}  {rel}" for rel, digest in sorted_entries]
    return header + "SHA-256 checksums:\n" + "\n".join(checksum_lines) + "\n"


def format_python_file(original_text, ordered_files, is_tuple=True):
    pattern = RE_BUILD_MOD_FILES if is_tuple else RE_TEST_MOD_FILES
    open_char = "(" if is_tuple else "["
    close_char = ")" if is_tuple else "]"

    lines = [f"BUNDLED_MOD_FILES = {open_char}"]
    for f in ordered_files:
        lines.append(f'    "{f}",')
    lines.append(close_char)
    new_block = "\n".join(lines)

    if not pattern.search(original_text):
        raise ValueError(f"Could not find BUNDLED_MOD_FILES block in file")

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

    mod_files = scan_bundled_mod_files(bundled_dir)
    disk_files_with_hashes = [(rel, sha256_file(p)) for rel, p in mod_files]
    disk_files_set = {rel for rel, _ in disk_files_with_hashes}

    old_source_txt = source_txt_path.read_text(encoding="utf-8")
    old_build_py = build_py_path.read_text(encoding="utf-8")
    old_test_py = test_py_path.read_text(encoding="utf-8")

    new_source_txt = format_source_txt(old_source_txt, disk_files_with_hashes)

    existing_build_files = parse_existing_bundled_files(old_build_py, is_tuple=True)
    ordered_files = order_mod_files(existing_build_files, disk_files_set)

    new_build_py = format_python_file(old_build_py, ordered_files, is_tuple=True)
    new_test_py = format_python_file(old_test_py, ordered_files, is_tuple=False)

    diffs = []
    for path, old, new in [
        (source_txt_path, old_source_txt, new_source_txt),
        (build_py_path, old_build_py, new_build_py),
        (test_py_path, old_test_py, new_test_py),
    ]:
        if old != new:
            diff = list(difflib.unified_diff(
                old.splitlines(keepends=True),
                new.splitlines(keepends=True),
                fromfile=str(path.relative_to(root)),
                tofile=str(path.relative_to(root)),
            ))
            diffs.append((path, "".join(diff)))

    if check:
        if diffs:
            print(f"Bundled mod manifests are out of date ({len(diffs)} file(s) differ):")
            for _, d in diffs:
                sys.stdout.write(d)
            return 1
        print("Bundled mod manifests are up to date.")
        return 0

    if diffs:
        source_txt_path.write_text(new_source_txt, encoding="utf-8")
        build_py_path.write_text(new_build_py, encoding="utf-8")
        test_py_path.write_text(new_test_py, encoding="utf-8")
        print(f"Updated {len(diffs)} bundled manifest file(s):")
        for path, _ in diffs:
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
