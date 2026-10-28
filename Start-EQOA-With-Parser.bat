@echo off
title EQOA Enhanced Client + Live Combat Parser
echo ======================================================================
echo Starting EverQuest Online Adventures - Enhanced Client + Combat Parser
echo ======================================================================
echo.

cd /d "%~dp0"

echo [1/2] Starting EQOA Combat Parser Companion (Direct Telemetry)...
python "%~dp0tools\parser_launch.py"
if errorlevel 1 (
    echo Parser startup failed. Check EQOA_PARSER_DIR and mods\eqoa_parser\mod.json.
    pause
    exit /b 1
)

echo [2/2] Launching EQOA Enhanced Native Client...
if exist "target\release\eqoa.exe" (
    start "" "target\release\eqoa.exe" %*
) else if exist "eqoa.exe" (
    start "" "eqoa.exe" %*
) else (
    echo Error: eqoa.exe not found!
    pause
)
