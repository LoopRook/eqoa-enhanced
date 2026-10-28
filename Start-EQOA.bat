@echo off
title EverQuest Online Adventures - Enhanced Client
echo ======================================================================
echo Starting EverQuest Online Adventures - Enhanced Native Client
echo ======================================================================
echo.

cd /d "%~dp0"
if exist "target\release\eqoa.exe" (
    start "" "target\release\eqoa.exe" %*
) else if exist "eqoa.exe" (
    start "" "eqoa.exe" %*
) else (
    echo Error: eqoa.exe not found!
    pause
)
