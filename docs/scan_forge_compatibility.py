#!/usr/bin/env python3
"""
Reproducible read-only scanner for FrontiersForge mod dependencies.
Extracts requires, ImGui/UiForge calls, FFI symbols, and resources via lexical analysis.
"""

import os
import re
import sys
import json

def scan_mod(base_dir, entry_rel, mod_dir_rel=None):
    entry_full = os.path.normpath(os.path.join(base_dir, entry_rel))
    files = [entry_full]
    private_modules = []

    if mod_dir_rel:
        mod_dir = os.path.normpath(os.path.join(base_dir, "scripts", mod_dir_rel))
        for root, _, filenames in os.walk(mod_dir):
            for fn in filenames:
                if fn.endswith(".lua"):
                    fp = os.path.normpath(os.path.join(root, fn))
                    if fp != entry_full and fp not in files:
                        files.append(fp)
                        private_modules.append(os.path.relpath(fp, base_dir).replace("\\", "/"))

    ff_imports = set()
    other_imports = set()
    imgui_calls = set()
    uiforge_calls = set()
    ffi_records = []
    referenced_resources = set()

    for f in files:
        rel_f = os.path.relpath(f, base_dir).replace("\\", "/")
        with open(f, "r", encoding="utf-8", errors="ignore") as fh:
            content = fh.read()

        for r in re.findall(r'require\s*\(\s*["\']([^"\']+)["\']\s*\)', content):
            if r.startswith("frontiers_forge."):
                ff_imports.add(r)
            else:
                other_imports.add(r)

        for ic in re.findall(r'ImGui\.([A-Za-z0-9_]+)', content):
            imgui_calls.add(f"ImGui.{ic}")

        for uc in re.findall(r'UiForge\.([A-Za-z0-9_]+)', content):
            uiforge_calls.add(f"UiForge.{uc}")

        has_ffi = "ffi" in other_imports or "ffi." in content or "EEmem" in content
        if has_ffi:
            matches = set()
            if "ffi" in other_imports:
                matches.add("require('ffi')")
            for m_f in re.findall(r'ffi\.([A-Za-z0-9_]+)', content):
                matches.add(f"ffi.{m_f}")
            if "EEmem" in content:
                matches.add("EEmem (PS2 EE RAM direct guest pointer dereference)")
            ffi_records.append({
                "file": rel_f,
                "symbols": sorted(list(matches))
            })

        for r in re.findall(r'["\']([^"\']+\.(?:png|jpg|jpeg|wav|mp3|json))["\']', content, re.IGNORECASE):
            clean_r = r.replace("\\\\", "/").replace("\\", "/")
            if not clean_r.startswith("Could not"):
                referenced_resources.add(clean_r)

    return {
        "sourceRelativePath": entry_rel,
        "packagePrivateModules": sorted(private_modules),
        "requiredFrontiersForgeImports": sorted(list(ff_imports)),
        "otherImports": sorted(list(other_imports)),
        "directImGuiCalls": sorted(list(imgui_calls)),
        "directUiForgeCalls": sorted(list(uiforge_calls)),
        "rawFfiDependencies": {
            "hasRawFfi": len(ffi_records) > 0,
            "details": ffi_records
        },
        "referencedResources": sorted(list(referenced_resources))
    }

if __name__ == "__main__":
    base = sys.argv[1] if len(sys.argv) > 1 else "run/forge-compat-reference"
    if not os.path.isdir(base):
        print(f"Directory not found: {base}", file=sys.stderr)
        sys.exit(1)
    # Quick sanity scan of ff_example
    result = scan_mod(base, "scripts/ff_example.lua")
    print(f"Successfully scanned {result['sourceRelativePath']}: {len(result['requiredFrontiersForgeImports'])} frontiers_forge imports, {len(result['directImGuiCalls'])} ImGui calls, hasRawFfi={result['rawFfiDependencies']['hasRawFfi']}")
