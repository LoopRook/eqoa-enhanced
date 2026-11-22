@echo off
rem Starts EQOA Enhanced with the performance log on, for this run only.
cd /d "%~dp0"
set EQOA_PERF=1
echo Performance log is on. The log is at %APPDATA%\eqoa-rs\eqoa.log
for %%F in ("EQOA Enhanced*.exe") do start "" "%%F" %*
