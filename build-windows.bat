@echo off
REM Windows release build for lancer-foundry.
REM Output: dist\lancer-foundry-windows-x86_64\bin\lancer-foundry.exe

echo === Building lancer-foundry (release) for Windows x86_64 ===
cargo build --release
if errorlevel 1 exit /b 1

echo === Packaging ===
set DIST=dist\lancer-foundry-windows-x86_64
if exist "%DIST%" rmdir /s /q "%DIST%"
mkdir "%DIST%"
mkdir "%DIST%\bin"

copy target\release\lancer-foundry.exe "%DIST%\bin\" >nul
copy LICENSE "%DIST%\" >nul 2>nul
copy README.md "%DIST%\" >nul 2>nul

echo === Done. Binary: %DIST%\bin ===
dir "%DIST%\bin"
exit /b 0
