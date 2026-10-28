@echo off
title EQOA Enhanced Client + Live Combat Parser
echo ======================================================================
echo Starting EverQuest Online Adventures - Enhanced Client + Combat Parser
echo ======================================================================
echo.

cd /d "%~dp0"

echo [1/2] Starting EQOA Combat Parser Companion (Direct Telemetry)...
start "EQOA Combat Parser" /d "C:\Users\hocka\Github Projects\EQOA Parser" python -m eqoa_parser --client

echo [2/2] Launching EQOA Enhanced Native Client...
if exist "target\release\eqoa.exe" (
    start "" "target\release\eqoa.exe" %*
) else if exist "eqoa.exe" (
    start "" "eqoa.exe" %*
) else (
    echo Error: eqoa.exe not found!
    pause
)
