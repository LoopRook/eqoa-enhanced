# Forge Early Preview Packaging Instructions

This directory contains the standalone packager script for the **EQOA FrontiersForge Early Test Preview**.

## Files

- `prepare-preview.ps1`: The PowerShell preview packaging script.

## Usage

### 1. Pre-Flight Validation (Without Packaging)

To verify that the FrontiersForge reference repository, pinned entity catalog, Lua scripts, PNG textures, and licenses are all present and match their pinned checksums, run with `-ValidateOnly`:

```powershell
.\target\forge-preview-prep\prepare-preview.ps1 -SourceRoot "C:\path\to\forge-compatibility" -ValidateOnly
```

During `-ValidateOnly`, if `SourceRoot` has uncommitted modifications, a warning is printed while allowing reference and pin checks to proceed.

### 2. Packaging an Already-Built Executable

When a verified executable has been built by root:

```powershell
.\target\forge-preview-prep\prepare-preview.ps1 `
    -SourceRoot "C:\path\to\forge-compatibility" `
    -ExecutablePath "C:\path\to\eqoa.exe"
```

Both `-SourceRoot` and `-ExecutablePath` are **mandatory** for packaging.

### Safety Guarantees & Packaging Architecture

1. **Explicit Source Root & Cleanliness**:
   - Resolves the main repository root via `git -c safe.directory=<SourceRoot> rev-parse --path-format=absolute --git-common-dir`.
   - Requires `SourceRoot` to be clean (`git status --porcelain` is empty) to guarantee release provenance (`sourceModified=false`).
   - Copies `LICENSE`, `FrontiersForge-MIT.txt`, and `SpaceMono-OFL.txt` directly from `SourceRoot`.

2. **Reference Git Pinning & Working Tree Check**:
   - Verifies the local FrontiersForge reference repository (`run/forge-compat-reference`) matches commit `4026d5cebcede24943ece8dc34833e584f46cd90`.
   - Runs `git diff --exit-code` and `git status --porcelain` on the selected 2 scripts (`screen_grid.lua`, `retro_health_hearts.lua`) and 4 PNGs (`heart_accent.png`, `heart_border.png`, `heart_fill.png`, `heart_highlight.png`) to ensure they are strictly unmodified relative to the pinned commit.
   - Performs byte-for-byte verification upon copying into `mods/FrontiersForge/scripts/...`.

3. **Output Directory Containment & Freshness**:
   - The output directory must be a subdirectory contained under `run/forge-compatibility-preview/` in the main repository.
   - Defaults to `run/forge-compatibility-preview/<sha12>/`.
   - **Fresh Artifact Rule**: If the target directory already exists with any files, the script aborts immediately without modifying anything, preventing accidental overwrites or corruption.

4. **Launcher Environment Isolation**:
   - The launcher script (`Play Forge Early Preview.cmd`) explicitly clears inherited developer/CI variables (`EQOA_FORGE_SPIKE`, `EQOA_FORGE_MODS`, `EQOA_OFFSCREEN`) before launching the game.
   - Sets isolated runtime configuration: `EQOA_CONFIG=%~dp0profile` and `EQOA_MAP_DATABASE=%~dp0map-entities.json`.
   - Preserves offline/no-update behavior (the updater is disabled by root's compilation configuration).

5. **Archive Hash & Path Validation**:
   - Builds `eqoa-forge-preview-<sha12>.zip` containing strictly the 15 allowlisted distribution files.
   - After creation, opens the zip and checks both entry paths and uncompressed stream SHA256 hashes against disk files.
   - Excludes all profiles, `.opt` files, log files, disc images, credentials, and source control data.
