@echo off
REM Compila Visualizzatore Immagini CP e crea l'installer. Da eseguire su Windows.
REM Requisiti: Rust (https://rustup.rs) e NSIS (https://nsis.sourceforge.io) installati.
setlocal
cd /d "%~dp0"

echo === Compilazione (release) ===
cargo build --release || goto :err

if not exist dist mkdir dist
copy /y target\release\visualizzatore-immagini-cp.exe dist\Visualizzatore-Immagini-CP-portatile.exe >nul

echo === Creazione installer ===
set MAKENSIS="C:\Program Files (x86)\NSIS\makensis.exe"
if not exist %MAKENSIS% set MAKENSIS=makensis
%MAKENSIS% installer\setup.nsi || goto :err

echo.
echo Fatto! I file sono nella cartella "dist".
pause
exit /b 0

:err
echo.
echo ERRORE durante la compilazione. Controlla i messaggi sopra.
pause
exit /b 1
