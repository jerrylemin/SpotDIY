@echo off
setlocal

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" set "VSWHERE=%ProgramFiles%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VSINSTALL="

if exist "%VSWHERE%" (
  for /f "usebackq tokens=*" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do if not defined VSINSTALL set "VSINSTALL=%%I"
)

if defined VSINSTALL if exist "%VSINSTALL%\Common7\Tools\VsDevCmd.bat" (
  call "%VSINSTALL%\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64
  if errorlevel 1 exit /b 1
) else if not defined VCToolsInstallDir (
  >&2 echo SpotDIY requires the Visual C++ Build Tools. Open an x64 Developer Command Prompt or install the C++ workload.
  exit /b 1
)

set "CARGO_BIN="
if defined CARGO_HOME if exist "%CARGO_HOME%\bin\cargo.exe" set "CARGO_BIN=%CARGO_HOME%\bin"
if not defined CARGO_BIN if defined USERPROFILE if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
if defined CARGO_BIN set "PATH=%CARGO_BIN%;%PATH%"

where cargo >nul 2>&1
if errorlevel 1 (
  >&2 echo SpotDIY requires Rust and Cargo. Install the Rustup MSVC toolchain or add its Cargo bin directory to PATH.
  exit /b 1
)

call "%~dp0..\node_modules\.bin\tauri.cmd" %*
exit /b %errorlevel%
