param(
    [string]$OutputDirectory,
    [string]$CatalogPath,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskRoot 'run/map-parity-preview' }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
if (-not $SkipBuild) {
    Push-Location $taskRoot
    try { & cargo build --release --locked -p eqoa; if ($LASTEXITCODE -ne 0) { throw 'Native build failed.' } }
    finally { Pop-Location }
}
$taskTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $taskRoot 'target' }
if (-not [IO.Path]::IsPathRooted($taskTarget)) { $taskTarget = Join-Path $taskRoot $taskTarget }
$taskSource = Join-Path $taskTarget 'release/eqoa.exe'
if (-not (Test-Path -LiteralPath $taskSource)) { throw "No executable at $taskSource" }
$taskSha = (Get-FileHash -LiteralPath $taskSource -Algorithm SHA256).Hash.ToLowerInvariant()
$taskName = "eqoa-map-preview-$($taskSha.Substring(0,12)).exe"
$taskDest = Join-Path $OutputDirectory $taskName
if (-not (Test-Path -LiteralPath $taskDest)) { Copy-Item -LiteralPath $taskSource -Destination $taskDest }
elseif ((Get-FileHash -LiteralPath $taskDest -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskSha) { throw 'Existing executable does not match its content address.' }

# The public historical catalog remains external/untracked. Pin both source and
# bytes; never silently substitute a new revision's data during a release.
$taskRevision = '4026d5cebcede24943ece8dc34833e584f46cd90'
$taskCatalogSha = '37be57d7996fbf7e7ef936c3e6ced1c3a14f611462346bc204eba5929cce11ca'
$taskCatalog = Join-Path $OutputDirectory 'map-entities.json'
if ($CatalogPath) {
    if ((Get-FileHash -LiteralPath $CatalogPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskCatalogSha) { throw 'Catalog does not match the pinned Forge revision.' }
    if ([IO.Path]::GetFullPath($CatalogPath) -ne $taskCatalog) { Copy-Item -LiteralPath $CatalogPath -Destination $taskCatalog -Force }
} elseif (-not (Test-Path -LiteralPath $taskCatalog)) {
    Invoke-WebRequest -Uri "https://raw.githubusercontent.com/mmvest/FrontiersForge/$taskRevision/scripts/resources/entity_database.json" -OutFile $taskCatalog
}
if ((Get-FileHash -LiteralPath $taskCatalog -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskCatalogSha) { throw 'Catalog checksum mismatch; refusing to package.' }
Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE') -Destination (Join-Path $OutputDirectory 'LICENSE') -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'docs/licenses/FrontiersForge-MIT.txt') -Destination (Join-Path $OutputDirectory 'FrontiersForge-MIT.txt') -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'crates/eqoa/resources/map-ui/OFL.txt') -Destination (Join-Path $OutputDirectory 'SpaceMono-OFL.txt') -Force
Copy-Item -LiteralPath (Join-Path $taskRoot 'docs/MAP-PLAYTEST.md') -Destination (Join-Path $OutputDirectory 'README.md') -Force
$taskLauncher = @"
@echo off
setlocal
cd /d "%~dp0"
set "EQOA_CONFIG=%~dp0profile"
set "EQOA_MAP_DATABASE=%~dp0map-entities.json"
start "" /D "%~dp0" "%~dp0$taskName"
endlocal
"@
foreach ($taskLauncherName in @('Play Map Preview.cmd','Play Map UI Preview.cmd')) {
    [IO.File]::WriteAllText((Join-Path $OutputDirectory $taskLauncherName), $taskLauncher.Replace("`r`n","`n").Replace("`n","`r`n"), [Text.UTF8Encoding]::new($false))
}
$taskCommit = (& git -C $taskRoot rev-parse HEAD).Trim()
$taskDirty = [bool](& git -C $taskRoot status --porcelain --untracked-files=normal)
$taskManifest = [ordered]@{ sourceCommit=$taskCommit; sourceModified=$taskDirty; executable=$taskName; executableSha256=$taskSha; catalogRevision=$taskRevision; catalogSha256=$taskCatalogSha; profile='profile/ (excluded from archive)' }
$taskManifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'release-manifest.json') -Encoding utf8
# Allowlist distribution files. Never archive profile/, game.opt, logs, disc
# images, cache, or arbitrary content from an existing preview directory.
$taskFiles = @($taskName,'LICENSE','map-entities.json','FrontiersForge-MIT.txt','SpaceMono-OFL.txt','README.md','Play Map Preview.cmd','Play Map UI Preview.cmd','release-manifest.json')
$taskZip = Join-Path $OutputDirectory "map-preview-$($taskSha.Substring(0,12)).zip"
Compress-Archive -LiteralPath ($taskFiles | ForEach-Object { Join-Path $OutputDirectory $_ }) -DestinationPath $taskZip -Force
Write-Output "Launcher: $(Join-Path $OutputDirectory 'Play Map Preview.cmd')"
Write-Output "Bundle: $taskZip"
Write-Output "SHA256: $taskSha"
